mod common;
use common::*;
use parcel_core::cancel::CancelToken;
use parcel_core::hash::sha256_file;
use parcel_core::limits::ResourceLimits;
use parcel_core::manifest::*;
use parcel_core::reader::*;
use parcel_core::writer::*;
use parcel_core::ParcelError;
use std::fs;

fn tree_hash(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![];
    fn walk(base: &std::path::Path, d: &std::path::Path, out: &mut Vec<(String, String)>) {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push((p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"), sha256_file(&p).unwrap().0));
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn manifest_json_roundtrip() {
    let mut m = Manifest::new("Round", "trip");
    let mut e = Entry::new(EntryKind::File, "x", "a.txt");
    e.storage_path = Some("content/a.txt".into());
    e.sha256 = Some("0".repeat(64));
    m.entries.push(e);
    let bytes = m.to_json().unwrap();
    let back = parse_manifest(&bytes, &ResourceLimits::default()).unwrap();
    assert_eq!(m, back);
}

#[test]
fn export_open_verify_extract_matches_originals_and_sources_untouched() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("src");
    let input = sample_input(&src);
    let before = tree_hash(&src);
    let dest = t.path().join("out/Sample.parcel");
    let report = build(input, &dest).unwrap();
    assert!(dest.exists());
    assert_eq!(report.entry_count, 6);
    assert_eq!(before, tree_hash(&src), "sources must be untouched");

    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    assert_eq!(r.manifest().name, "Sample");
    assert_eq!(r.notes().unwrap(), "workspace notes");
    assert_eq!(r.manifest().references.len(), 1);
    let rep = r.verify(&CancelToken::new(), &noop).unwrap();
    assert!(rep.ok, "{}", rep.to_text());
    assert_eq!(rep.checked, 4);
    let png = r.manifest().entry_by_id("e2").unwrap();
    assert_eq!(png.mime.as_deref(), Some("image/png"));

    let out = t.path().join("extracted");
    let x = r.extract(&out, &ExtractOptions::default(), &CancelToken::new(), &noop).unwrap();
    assert!(!x.written.is_empty());
    assert_eq!(fs::read(out.join("Docs/report.txt")).unwrap(), fs::read(src.join("a/report.txt")).unwrap());
    assert_eq!(fs::read(out.join("Docs/image.png")).unwrap(), fs::read(src.join("b/image.png")).unwrap());
    assert_eq!(fs::read(out.join("data.csv")).unwrap(), fs::read(src.join("c/data.csv")).unwrap());
    assert!(out.join("Notes/readme.md").exists());
    assert!(out.join("Links/rust.url.txt").exists());
    assert_eq!(before, tree_hash(&src));
    // No leftover temp files anywhere.
    for (p, _) in tree_hash(t.path()) {
        assert!(!p.contains("parcel-tmp") && !p.contains("parcel-part"), "leftover {p}");
    }
}

#[test]
fn export_never_overwrites_without_confirmation() {
    let t = tempfile::tempdir().unwrap();
    let dest = t.path().join("p.parcel");
    fs::write(&dest, b"precious").unwrap();
    let err = build(sample_input(&t.path().join("s")), &dest).unwrap_err();
    assert!(matches!(err, ParcelError::AlreadyExists(_)));
    assert_eq!(fs::read(&dest).unwrap(), b"precious");
    let opts = ExportOptions { overwrite: true, ..Default::default() };
    build_package(sample_input(&t.path().join("s2")), &dest, opts, &CancelToken::new(), &noop).unwrap();
    assert!(PackageReader::open(&dest, &ResourceLimits::default()).is_ok());
}

#[test]
fn cancelled_export_leaves_nothing_behind() {
    let t = tempfile::tempdir().unwrap();
    let input = sample_input(&t.path().join("s"));
    let dest = t.path().join("out/p.parcel");
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = build_package(input, &dest, ExportOptions::default(), &cancel, &noop).unwrap_err();
    assert!(matches!(err, ParcelError::Cancelled));
    assert!(!dest.exists());
    let leftovers: Vec<_> = fs::read_dir(t.path().join("out")).map(|d| d.map(|e| e.unwrap().path()).collect()).unwrap_or_default();
    assert!(leftovers.is_empty(), "leftovers: {leftovers:?}");
}

#[test]
fn cancel_mid_export_via_progress_callback() {
    let t = tempfile::tempdir().unwrap();
    let big = write(t.path(), "big.bin", &vec![7u8; 8 * 1024 * 1024]);
    let mut input = sample_input(&t.path().join("s"));
    input.items.push(BuildItem { entry: Entry::new(EntryKind::File, "big", "big.bin"), content: ItemContent::File(big) });
    let dest = t.path().join("p.parcel");
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    let cb = move |p: &parcel_core::cancel::Progress| {
        if p.current_item == "big.bin" && p.bytes_done > 0 {
            c2.cancel();
        }
    };
    let err = build_package(input, &dest, ExportOptions::default(), &cancel, &cb).unwrap_err();
    assert!(matches!(err, ParcelError::Cancelled));
    assert!(!dest.exists());
    assert_eq!(
        fs::read_dir(t.path()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("parcel-tmp")).count(),
        0
    );
}

#[test]
fn large_file_streams_and_verifies() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("large.bin");
    {
        use std::io::Write;
        let mut f = fs::File::create(&p).unwrap();
        let mut chunk = vec![0u8; 1024 * 1024];
        for i in 0..150u32 {
            for (j, b) in chunk.iter_mut().enumerate() {
                *b = ((i as usize * 31 + j * 7) % 251) as u8;
            }
            f.write_all(&chunk).unwrap();
        }
    }
    let mut base = Manifest::new("Large", "");
    base.entries.clear();
    let input = BuildInput {
        base,
        items: vec![BuildItem { entry: Entry::new(EntryKind::File, "L", "large.bin"), content: ItemContent::File(p.clone()) }],
        workspace_notes: String::new(),
        thumbnail_png: None,
    };
    let dest = t.path().join("l.parcel");
    let rep = build(input, &dest).unwrap();
    assert_eq!(rep.content_size, 150 * 1024 * 1024);
    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    assert!(r.verify(&CancelToken::new(), &noop).unwrap().ok);
    let out = t.path().join("x");
    r.extract(&out, &ExtractOptions::default(), &CancelToken::new(), &noop).unwrap();
    assert_eq!(sha256_file(&p).unwrap(), sha256_file(&out.join("large.bin")).unwrap());
}

#[test]
fn selected_extraction_includes_folder_descendants_only() {
    let t = tempfile::tempdir().unwrap();
    let dest = t.path().join("p.parcel");
    build(sample_input(&t.path().join("s")), &dest).unwrap();
    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    let out = t.path().join("x");
    let opts = ExtractOptions { selection: Some(vec!["f1".into()]), ..Default::default() };
    r.extract(&out, &opts, &CancelToken::new(), &noop).unwrap();
    assert!(out.join("Docs/report.txt").exists() && out.join("Docs/image.png").exists());
    assert!(!out.join("data.csv").exists());
    let out2 = t.path().join("y");
    let opts = ExtractOptions { selection: Some(vec!["e3".into()]), ..Default::default() };
    r.extract(&out2, &opts, &CancelToken::new(), &noop).unwrap();
    assert!(out2.join("data.csv").exists() && !out2.join("Docs").exists());
}

#[test]
fn extraction_conflict_policies_never_silently_overwrite() {
    let t = tempfile::tempdir().unwrap();
    let dest = t.path().join("p.parcel");
    build(sample_input(&t.path().join("s")), &dest).unwrap();
    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    let out = t.path().join("x");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("data.csv"), b"mine").unwrap();
    // Fail: aborts before writing anything.
    let err = r.extract(&out, &ExtractOptions::default(), &CancelToken::new(), &noop).unwrap_err();
    assert!(matches!(err, ParcelError::AlreadyExists(_)));
    assert!(!out.join("Docs").exists());
    assert_eq!(fs::read(out.join("data.csv")).unwrap(), b"mine");
    // Skip
    let opts = ExtractOptions { conflict: ConflictPolicy::Skip, ..Default::default() };
    let rep = r.extract(&out, &opts, &CancelToken::new(), &noop).unwrap();
    assert_eq!(rep.skipped, vec!["data.csv".to_string()]);
    assert_eq!(fs::read(out.join("data.csv")).unwrap(), b"mine");
    // Rename
    let opts = ExtractOptions { conflict: ConflictPolicy::Rename, ..Default::default() };
    r.extract(&out, &opts, &CancelToken::new(), &noop).unwrap();
    assert_eq!(fs::read(out.join("data.csv")).unwrap(), b"mine");
    assert!(out.join("data (1).csv").exists());
}

#[test]
fn path_traversal_in_archive_is_rejected_on_open() {
    let t = tempfile::tempdir().unwrap();
    for bad in ["../evil.txt", "/abs.txt", "content/../../evil", "C:/win.txt", "a\\b.txt", "content/CON.txt", "content/a/./b"] {
        let p = t.path().join("bad.parcel");
        raw_zip(&p, &[("manifest.json", b"{}"), (bad, b"x")]);
        let err = PackageReader::open(&p, &ResourceLimits::default()).err().unwrap();
        assert!(matches!(err, ParcelError::UnsafePath(_)), "{bad}: {err:?}");
    }
}

#[test]
fn traversal_in_manifest_paths_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("good.parcel");
    build(sample_input(&t.path().join("s")), &good).unwrap();
    for (field, value) in
        [("path", "../../escape.txt"), ("path", "/etc/passwd"), ("storage_path", "../manifest.json"), ("storage_path", "metadata/notes.md")]
    {
        let bad = t.path().join("bad.parcel");
        rewrite_manifest(&good, &bad, |v| {
            v["entries"][1][field] = serde_json::json!(value);
        });
        assert!(PackageReader::open(&bad, &ResourceLimits::default()).is_err(), "{field}={value} accepted");
    }
}

#[test]
fn symlinks_in_archive_are_rejected() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("sym.parcel");
    let f = fs::File::create(&p).unwrap();
    let mut z = zip::ZipWriter::new(f);
    z.add_symlink("content/link", "/etc/passwd", zip::write::SimpleFileOptions::default()).unwrap();
    z.finish().unwrap();
    let err = PackageReader::open(&p, &ResourceLimits::default()).err().unwrap();
    assert!(matches!(err, ParcelError::UnsafePath(_)), "{err:?}");
}

#[test]
fn conflicting_duplicate_paths_rejected() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("dup.parcel");
    raw_zip(&p, &[("manifest.json", b"{}"), ("content/A.txt", b"1"), ("content/a.txt", b"2")]);
    assert!(PackageReader::open(&p, &ResourceLimits::default()).is_err());
    // Duplicate entry paths in a manifest.
    let good = t.path().join("good.parcel");
    build(sample_input(&t.path().join("s")), &good).unwrap();
    let bad = t.path().join("bad.parcel");
    rewrite_manifest(&good, &bad, |v| {
        v["entries"][2]["path"] = serde_json::json!("DOCS/REPORT.TXT");
    });
    assert!(matches!(PackageReader::open(&bad, &ResourceLimits::default()).err().unwrap(), ParcelError::InvalidManifest(_)));
}

#[test]
fn zip_bomb_and_size_limits_enforced() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("bomb.parcel");
    let f = fs::File::create(&p).unwrap();
    let mut z = zip::ZipWriter::new(f);
    z.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut z, b"{}").unwrap();
    z.start_file("content/zeros.bin", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))
        .unwrap();
    std::io::Write::write_all(&mut z, &vec![0u8; 12 * 1024 * 1024]).unwrap();
    z.finish().unwrap();
    let limits = ResourceLimits { max_compression_ratio: 100, ..Default::default() };
    let err = PackageReader::open(&p, &limits).err().unwrap();
    assert!(matches!(err, ParcelError::LimitExceeded(_)), "{err:?}");
    let limits = ResourceLimits { max_total_uncompressed: 1024 * 1024, max_compression_ratio: 100_000, ..Default::default() };
    assert!(matches!(PackageReader::open(&p, &limits).err().unwrap(), ParcelError::LimitExceeded(_)));
    let limits = ResourceLimits { max_entries: 0, ..Default::default() };
    assert!(matches!(PackageReader::open(&p, &limits).err().unwrap(), ParcelError::LimitExceeded(_)));
}

#[test]
fn export_respects_entry_limit() {
    let t = tempfile::tempdir().unwrap();
    let opts = ExportOptions { limits: ResourceLimits { max_entries: 2, ..Default::default() }, ..Default::default() };
    let err = build_package(sample_input(&t.path().join("s")), &t.path().join("p.parcel"), opts, &CancelToken::new(), &noop).unwrap_err();
    assert!(matches!(err, ParcelError::LimitExceeded(_)));
    assert!(!t.path().join("p.parcel").exists());
}

#[test]
fn malformed_and_invalid_manifests_rejected() {
    let lim = ResourceLimits::default();
    for bad in [&b"not json"[..], b"{}", br#"{"format":"other","format_version":"1.0"}"#, br#"{"format":"parcel","format_version":"x.y"}"#]
    {
        assert!(parse_manifest(bad, &lim).is_err());
    }
    let err = parse_manifest(br#"{"format":"parcel","format_version":"2.0"}"#, &lim).unwrap_err();
    assert!(matches!(err, ParcelError::UnsupportedVersion(_)));
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("g.parcel");
    build(sample_input(&t.path().join("s")), &good).unwrap();
    type Edit = Box<dyn Fn(&mut serde_json::Value)>;
    let cases: Vec<(&str, Edit)> = vec![
        ("bad uuid", Box::new(|v| v["package_id"] = "nope".into())),
        ("bad sha", Box::new(|v| v["entries"][1]["sha256"] = "zz".into())),
        (
            "missing sha",
            Box::new(|v| {
                v["entries"][1].as_object_mut().unwrap().remove("sha256");
            }),
        ),
        ("js url", Box::new(|v| v["references"][0]["url"] = "javascript:alert(1)".into())),
        ("file url", Box::new(|v| v["entries"][5]["url"] = "file:///C:/Windows".into())),
        ("dangling ref", Box::new(|v| v["references"][0]["entry_id"] = "ghost".into())),
        ("folder with content", Box::new(|v| v["entries"][0]["size"] = 5.into())),
        ("dup id", Box::new(|v| v["entries"][2]["id"] = "e1".into())),
        ("nested under file", Box::new(|v| v["entries"][2]["path"] = "data.csv/inside.txt".into())),
        ("empty name", Box::new(|v| v["name"] = "  ".into())),
        ("empty metadata key", Box::new(|v| v["entries"][1]["metadata"] = serde_json::json!({"": "x"}))),
    ];
    for (label, edit) in cases {
        let bad = t.path().join("b.parcel");
        rewrite_manifest(&good, &bad, |v| edit(v));
        assert!(PackageReader::open(&bad, &lim).is_err(), "{label} should be rejected");
    }
}

#[test]
fn missing_manifest_and_non_zip_are_not_packages() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("a.parcel");
    raw_zip(&p, &[("hello.txt", b"hi")]);
    assert!(matches!(PackageReader::open(&p, &ResourceLimits::default()).err().unwrap(), ParcelError::NotAPackage(_)));
    let q = t.path().join("b.parcel");
    fs::write(&q, b"this is not a zip file at all").unwrap();
    assert!(matches!(PackageReader::open(&q, &ResourceLimits::default()).err().unwrap(), ParcelError::NotAPackage(_)));
    let trunc = t.path().join("c.parcel");
    build(sample_input(&t.path().join("s")), &t.path().join("ok.parcel")).unwrap();
    let bytes = fs::read(t.path().join("ok.parcel")).unwrap();
    fs::write(&trunc, &bytes[..bytes.len() / 2]).unwrap();
    assert!(PackageReader::open(&trunc, &ResourceLimits::default()).is_err());
}

#[test]
fn corrupted_stored_bytes_detected() {
    let t = tempfile::tempdir().unwrap();
    let payload = b"UNIQUE_MARKER_PAYLOAD_0123456789_ABCDEFGHIJKLMNOP".to_vec();
    let mut input = sample_input(&t.path().join("s"));
    input.items.push(BuildItem { entry: Entry::new(EntryKind::File, "m", "marker.bin"), content: ItemContent::Bytes(payload.clone()) });
    let dest = t.path().join("p.parcel");
    let opts = ExportOptions { compress: false, ..Default::default() };
    build_package(input, &dest, opts, &CancelToken::new(), &noop).unwrap();
    let mut bytes = fs::read(&dest).unwrap();
    let pos = bytes.windows(payload.len()).position(|w| w == payload.as_slice()).unwrap();
    bytes[pos + 10] ^= 0xFF;
    fs::write(&dest, &bytes).unwrap();
    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    let rep = r.verify(&CancelToken::new(), &noop).unwrap();
    assert!(!rep.ok);
    assert_eq!(rep.failures.len(), 1);
    assert_eq!(rep.failures[0].path, "marker.bin");
    // Extraction refuses corrupt content and leaves no partial file.
    let out = t.path().join("x");
    let opts = ExtractOptions { selection: Some(vec!["m".into()]), ..Default::default() };
    assert!(r.extract(&out, &opts, &CancelToken::new(), &noop).is_err());
    assert!(!out.join("marker.bin").exists());
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0, "no partial files");
}

#[test]
fn wrong_checksum_in_manifest_detected_even_with_valid_crc() {
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("g.parcel");
    build(sample_input(&t.path().join("s")), &good).unwrap();
    let bad = t.path().join("b.parcel");
    rewrite_manifest(&good, &bad, |v| v["entries"][1]["sha256"] = "a".repeat(64).into());
    let r = PackageReader::open(&bad, &ResourceLimits::default()).unwrap();
    let rep = r.verify(&CancelToken::new(), &noop).unwrap();
    assert!(!rep.ok);
    assert_eq!(rep.failures[0].result, CheckStatus::HashMismatch);
    assert!(rep.to_text().contains("FAILED"));
}

#[test]
fn size_lie_in_manifest_detected() {
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("g.parcel");
    build(sample_input(&t.path().join("s")), &good).unwrap();
    let bad = t.path().join("b.parcel");
    rewrite_manifest(&good, &bad, |v| v["entries"][1]["size"] = 3.into());
    let r = PackageReader::open(&bad, &ResourceLimits::default()).unwrap();
    let rep = r.verify(&CancelToken::new(), &noop).unwrap();
    assert!(!rep.ok);
    assert!(matches!(rep.failures[0].result, CheckStatus::SizeMismatch { .. }));
}

#[test]
fn unlisted_and_missing_members_reported() {
    let t = tempfile::tempdir().unwrap();
    let good = t.path().join("g.parcel");
    build(sample_input(&t.path().join("s")), &good).unwrap();
    let r = PackageReader::open(&good, &ResourceLimits::default()).unwrap();
    assert!(r.verify(&CancelToken::new(), &noop).unwrap().unlisted.is_empty());
    let bad = t.path().join("b.parcel");
    rewrite_manifest(&good, &bad, |v| v["entries"][1]["storage_path"] = "content/not-there.txt".into());
    let r = PackageReader::open(&bad, &ResourceLimits::default()).unwrap();
    let rep = r.verify(&CancelToken::new(), &noop).unwrap();
    assert!(!rep.ok);
    assert_eq!(rep.failures[0].result, CheckStatus::Missing);
    assert!(rep.unlisted.iter().any(|u| u == "content/Docs/report.txt"));
}

#[test]
fn zip_import_converts_to_valid_parcel() {
    let t = tempfile::tempdir().unwrap();
    let z = t.path().join("in.zip");
    raw_zip(
        &z,
        &[("proj/readme.txt", b"hello"), ("proj/src/main.rs", b"fn main(){}"), ("top.txt", b"top"), ("__MACOSX/._top.txt", b"junk")],
    );
    let dest = t.path().join("out.parcel");
    let rep =
        parcel_core::zipimport::convert_zip_to_parcel(&z, &dest, "Imported", ExportOptions::default(), &CancelToken::new(), &noop).unwrap();
    assert!(rep.warnings.iter().any(|w| w.contains("macOS")));
    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    assert!(r.verify(&CancelToken::new(), &noop).unwrap().ok);
    let paths: Vec<_> = r.manifest().entries.iter().map(|e| e.path.clone()).collect();
    for p in ["proj", "proj/src", "proj/readme.txt", "proj/src/main.rs", "top.txt"] {
        assert!(paths.contains(&p.to_string()), "missing {p}: {paths:?}");
    }
    assert_eq!(paths.len(), 5);
    let out = t.path().join("x");
    r.extract(&out, &ExtractOptions::default(), &CancelToken::new(), &noop).unwrap();
    assert_eq!(fs::read(out.join("proj/src/main.rs")).unwrap(), b"fn main(){}");
}

#[test]
fn zip_import_rejects_hostile_archives() {
    let t = tempfile::tempdir().unwrap();
    let dest = t.path().join("o.parcel");
    for name in ["../evil.txt", "/abs.txt", "a/../../b.txt"] {
        let z = t.path().join("in.zip");
        raw_zip(&z, &[(name, b"x")]);
        let err = parcel_core::zipimport::convert_zip_to_parcel(&z, &dest, "X", ExportOptions::default(), &CancelToken::new(), &noop)
            .err()
            .unwrap();
        assert!(matches!(err, ParcelError::UnsafePath(_)), "{name}: {err:?}");
        assert!(!dest.exists());
    }
    let z = t.path().join("sym.zip");
    let f = fs::File::create(&z).unwrap();
    let mut w = zip::ZipWriter::new(f);
    w.add_symlink("link", "/etc/passwd", zip::write::SimpleFileOptions::default()).unwrap();
    w.start_file("ok.txt", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut w, b"ok").unwrap();
    w.finish().unwrap();
    let rep = parcel_core::zipimport::convert_zip_to_parcel(&z, &dest, "X", ExportOptions::default(), &CancelToken::new(), &noop).unwrap();
    assert!(rep.warnings.iter().any(|w| w.contains("symbolic")));
    assert_eq!(rep.build.entry_count, 1);
    // ZIP bomb during import.
    let bomb = t.path().join("bomb.zip");
    let f = fs::File::create(&bomb).unwrap();
    let mut w = zip::ZipWriter::new(f);
    w.start_file("z.bin", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
    std::io::Write::write_all(&mut w, &vec![0u8; 12 * 1024 * 1024]).unwrap();
    w.finish().unwrap();
    let opts = ExportOptions { limits: ResourceLimits { max_compression_ratio: 100, ..Default::default() }, ..Default::default() };
    let d2 = t.path().join("o2.parcel");
    let err = parcel_core::zipimport::convert_zip_to_parcel(&bomb, &d2, "B", opts, &CancelToken::new(), &noop).err().unwrap();
    assert!(matches!(err, ParcelError::LimitExceeded(_)));
    assert!(!d2.exists());
}

#[cfg(unix)]
#[test]
fn extraction_refuses_to_follow_existing_symlinked_directories() {
    let t = tempfile::tempdir().unwrap();
    let dest = t.path().join("p.parcel");
    build(sample_input(&t.path().join("s")), &dest).unwrap();
    let outside = t.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let out = t.path().join("x");
    fs::create_dir_all(&out).unwrap();
    std::os::unix::fs::symlink(&outside, out.join("Docs")).unwrap();
    let mut r = PackageReader::open(&dest, &ResourceLimits::default()).unwrap();
    let opts = ExtractOptions { conflict: ConflictPolicy::Rename, ..Default::default() };
    assert!(r.extract(&out, &opts, &CancelToken::new(), &noop).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0, "nothing may be written through the symlink");
}

#[test]
fn source_changing_during_export_is_detected() {
    // Declared length differs from bytes actually read.
    let t = tempfile::tempdir().unwrap();
    let tok = CancelToken::new();
    let mut w =
        PackageWriter::create(&t.path().join("p.parcel"), Manifest::new("X", ""), ExportOptions::default(), 1, 10, &tok, &noop).unwrap();
    let mut cur = std::io::Cursor::new(vec![1u8; 10]);
    let err = w.add_stream(Entry::new(EntryKind::File, "a", "a.bin"), &mut cur, Some(5)).unwrap_err();
    assert!(err.to_string().contains("changed while exporting"));
    drop(w);
    assert!(!t.path().join("p.parcel").exists());
}
