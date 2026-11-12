//! Regression tests for the 1.1.0 fixes: diagnostics, atomic import, managed-file cleanup, atomic add, launch parsing.
use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::limits::ResourceLimits;
use parcel_core::manifest::{Entry as MEntry, EntryKind, Manifest};
use parcel_core::writer::{ExportOptions, PackageWriter};
use parcel_workspace::collect::{scan_paths, ScanReport, ScannedItem};
use parcel_workspace::import::StagedImport;
use parcel_workspace::launch::package_paths_from_args;
use parcel_workspace::models::RefInput;
use parcel_workspace::ops::ExportRequest;
use parcel_workspace::{Store, WsError};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn noop(_: &Progress) {}
fn w(dir: &Path, rel: &str, data: &[u8]) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, data).unwrap();
    p
}
fn add(s: &mut Store, ws: &str, paths: &[PathBuf]) {
    let scan = scan_paths(paths, &CancelToken::new(), &noop).unwrap();
    s.add_scanned(ws, None, &scan).unwrap();
}
fn export(s: &Store, ws: &str, dest: &Path) {
    s.export_workspace(ws, &ExportRequest { dest: dest.to_path_buf(), ..Default::default() }, &CancelToken::new(), &noop).unwrap();
}
fn managed_children(dir: &Path) -> Vec<String> {
    fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
}
/// A package with files, a note attached to a file, a link, a reference and a dependency.
fn rich_package(t: &Path) -> PathBuf {
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Rich", "desc", None).unwrap().id;
    let a_txt = w(&t.join("src"), "a.txt", b"alpha");
    w(&t.join("src"), "dir/b.txt", b"beta beta");
    add(&mut s, &ws, &[a_txt, t.join("src/dir")]);
    let es = s.list_entries(&ws).unwrap();
    let a = es.iter().find(|e| e.name == "a.txt").unwrap().id.clone();
    let b = es.iter().find(|e| e.name == "b.txt").unwrap().id.clone();
    s.create_note(&ws, None, "n", "note body", Some(&a)).unwrap();
    s.create_link(&ws, None, "site", "https://example.com", "").unwrap();
    s.add_reference(
        &ws,
        RefInput {
            title: "r".into(),
            url: "https://example.com/r".into(),
            description: String::new(),
            category: "c".into(),
            entry_id: Some(b.clone()),
        },
    )
    .unwrap();
    s.add_dependency(&ws, &a, "entry", &b, "needs b", true).unwrap();
    s.add_dependency(&ws, &a, "external", "Python 3.12", "", false).unwrap();
    s.update_workspace(&ws, parcel_workspace::models::WorkspaceUpdate { notes: Some("ws notes".into()), ..Default::default() }).unwrap();
    let p = t.join("rich.parcel");
    export(&s, &ws, &p);
    p
}
fn import(s: &mut Store, pkg: &Path, managed: &Path) -> parcel_workspace::models::Workspace {
    s.import_package(pkg, managed, &ResourceLimits::default(), &CancelToken::new(), &noop).unwrap()
}

// ---------- defect 1: diagnostics must survive missing sources ----------
#[test]
fn dependency_diagnostics_are_reported_even_when_sources_are_missing() {
    let t = tempfile::tempdir().unwrap();
    let a = w(t.path(), "a.txt", b"A");
    let b = w(t.path(), "b.txt", b"B");
    let c = w(t.path(), "c.txt", b"C");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("D", "", None).unwrap().id;
    add(&mut s, &ws, &[a.clone(), b, c]);
    let id = |n: &str| s.list_entries(&ws).unwrap().into_iter().find(|e| e.name == n).unwrap().id;
    let (ia, ib, ic) = (id("a.txt"), id("b.txt"), id("c.txt"));
    s.add_dependency(&ws, &ia, "entry", &ib, "", true).unwrap();
    s.add_dependency(&ws, &ib, "entry", &ia, "", true).unwrap(); // cycle
    s.add_dependency(&ws, &ia, "entry", &ic, "", true).unwrap();
    s.add_dependency(&ws, &ia, "external", "Python 3.12", "", false).unwrap();
    s.delete_entries(&[ic]).unwrap(); // leaves a dangling dependency target
    fs::remove_file(&a).unwrap(); // a source goes missing

    // The old code path (export preparation) refuses to run in this state...
    let req = ExportRequest { dest: t.path().join("x.parcel"), accept_changed: true, ..Default::default() };
    assert!(matches!(s.prepare_export(&ws, &req).err().unwrap(), WsError::SourceProblems { .. }));
    // ...but the dedicated diagnostics keep working.
    let codes: Vec<String> = s.diagnose_dependencies(&ws).unwrap().into_iter().map(|d| d.code).collect();
    assert!(codes.contains(&"dependency.cycle".to_string()), "{codes:?}");
    assert!(codes.contains(&"dependency.missing_target".to_string()), "{codes:?}");
    assert!(codes.contains(&"dependency.external".to_string()), "{codes:?}");
}

// ---------- defect 3: atomic, lock-free-able import ----------
#[test]
fn import_round_trips_everything_and_uses_the_managed_folder() {
    let t = tempfile::tempdir().unwrap();
    let pkg = rich_package(t.path());
    let managed = t.path().join("managed");
    let mut s = Store::open_in_memory().unwrap();
    let ws = import(&mut s, &pkg, &managed);
    assert_eq!((ws.name.as_str(), ws.revision, ws.notes.as_str()), ("Rich", 1, "ws notes"));
    let es = s.list_entries(&ws.id).unwrap();
    assert_eq!(es.len(), 5, "a, dir, b, note, link: {:?}", es.iter().map(|e| &e.name).collect::<Vec<_>>());
    let file = es.iter().find(|e| e.name == "b.txt").unwrap();
    assert_eq!(file.origin, "imported");
    assert_eq!(fs::read(file.source_path.as_ref().unwrap()).unwrap(), b"beta beta");
    assert_eq!(es.iter().find(|e| e.kind == "note").unwrap().content.as_deref(), Some("note body"));
    let note = es.iter().find(|e| e.kind == "note").unwrap();
    assert_eq!(es.iter().find(|e| Some(&e.id) == note.attached_to.as_ref()).unwrap().name, "a.txt");
    assert_eq!(s.list_references(&ws.id).unwrap().len(), 1);
    assert_eq!(s.list_dependencies(&ws.id).unwrap().len(), 2);
    assert!(s
        .check_sources(&ws.id, true, &CancelToken::new())
        .unwrap()
        .iter()
        .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)));
    // Only files are extracted; notes live in the database.
    assert_eq!(managed_children(&managed), vec![ws.id.clone()]);
    let on_disk: Vec<_> = walk(&managed.join(&ws.id));
    assert_eq!(on_disk.len(), 2, "{on_disk:?}");
}
fn walk(d: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    for e in fs::read_dir(d).unwrap().flatten() {
        if e.path().is_dir() {
            v.extend(walk(&e.path()))
        } else {
            v.push(e.path())
        }
    }
    v
}

#[test]
fn import_creates_missing_parent_folders_for_packages_without_folder_entries() {
    let t = tempfile::tempdir().unwrap();
    let pkg = t.path().join("flat.parcel");
    let tok = CancelToken::new();
    let mut pw = PackageWriter::create(&pkg, Manifest::new("Flat", ""), ExportOptions::default(), 2, 10, &tok, &noop).unwrap();
    pw.add_bytes(MEntry::new(EntryKind::File, "1", "A/B/c.txt"), b"c").unwrap();
    pw.add_bytes(MEntry::new(EntryKind::File, "2", "A/d.txt"), b"d").unwrap();
    pw.finish("", None).unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = import(&mut s, &pkg, &t.path().join("m"));
    let mut paths: Vec<String> = s.entry_paths(&ws.id).unwrap().into_values().collect();
    paths.sort();
    assert_eq!(paths, vec!["A", "A/B", "A/B/c.txt", "A/d.txt"]);
}

#[test]
fn committing_twice_fails_atomically_without_partial_rows() {
    let t = tempfile::tempdir().unwrap();
    let pkg = rich_package(t.path());
    let managed = t.path().join("managed");
    let mut s = Store::open_in_memory().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let staged = StagedImport::prepare(&pkg, &managed, &id, &ResourceLimits::default(), &CancelToken::new(), &noop).unwrap();
    s.commit_import(&staged).unwrap();
    let count =
        |s: &Store| (s.list_workspaces(false).unwrap().len(), s.list_entries(&id).unwrap().len(), s.list_references(&id).unwrap().len());
    let before = count(&s);
    assert!(s.commit_import(&staged).is_err(), "duplicate workspace id must be rejected");
    assert_eq!(before, count(&s), "a failed commit must leave nothing behind");
}

#[test]
fn failed_import_leaves_no_workspace_and_no_extracted_files() {
    let t = tempfile::tempdir().unwrap();
    // Two stored files; the SECOND is corrupted so the first is extracted before the failure.
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Corrupt me", "", None).unwrap().id;
    s.update_workspace(
        &ws,
        parcel_workspace::models::WorkspaceUpdate { export_profile_id: Some(Some("prof-fast".into())), ..Default::default() },
    )
    .unwrap();
    let marker = b"UNIQUE_IMPORT_MARKER_0123456789_ABCDEFGHIJKLMNOP";
    add(&mut s, &ws, &[w(&t.path().join("src"), "1-first.txt", b"first file"), w(&t.path().join("src"), "2-second.txt", marker)]);
    let pkg = t.path().join("bad.parcel");
    export(&s, &ws, &pkg);
    let mut bytes = fs::read(&pkg).unwrap();
    let pos = bytes.windows(marker.len()).position(|x| x == marker).unwrap();
    bytes[pos + 7] ^= 0xFF;
    fs::write(&pkg, bytes).unwrap();

    let managed = t.path().join("managed");
    let mut target = Store::open_in_memory().unwrap();
    let err = target.import_package(&pkg, &managed, &ResourceLimits::default(), &CancelToken::new(), &noop).err().unwrap();
    assert!(matches!(err, WsError::Core(_)), "{err:?}");
    assert!(target.list_workspaces(false).unwrap().is_empty());
    assert!(target.list_workspaces(true).unwrap().is_empty());
    assert!(managed_children(&managed).is_empty(), "extracted files must be removed: {:?}", managed_children(&managed));
}

#[test]
fn cancelled_import_leaves_no_workspace_and_no_extracted_files() {
    let t = tempfile::tempdir().unwrap();
    let pkg = rich_package(t.path());
    let managed = t.path().join("managed");
    let mut s = Store::open_in_memory().unwrap();
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    let cb = move |p: &Progress| {
        if p.phase == "extracting" {
            c2.cancel();
        }
    };
    let err = s.import_package(&pkg, &managed, &ResourceLimits::default(), &cancel, &cb).err().unwrap();
    assert!(matches!(err, WsError::Core(parcel_core::ParcelError::Cancelled)), "{err:?}");
    assert!(s.list_workspaces(false).unwrap().is_empty());
    assert!(managed_children(&managed).is_empty());
    // Cancelling between staging and commit also leaves nothing.
    let cancel2 = CancelToken::new();
    let staged =
        StagedImport::prepare(&pkg, &managed, &uuid::Uuid::new_v4().to_string(), &ResourceLimits::default(), &cancel2, &noop).unwrap();
    staged.discard();
    assert!(managed_children(&managed).is_empty());
}

#[test]
fn hostile_package_is_refused_before_anything_is_written() {
    let t = tempfile::tempdir().unwrap();
    let bad = t.path().join("evil.parcel");
    {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
        z.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"{}").unwrap();
        z.start_file("../escape.txt", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
    }
    let managed = t.path().join("managed");
    let mut s = Store::open_in_memory().unwrap();
    assert!(s.import_package(&bad, &managed, &ResourceLimits::default(), &CancelToken::new(), &noop).is_err());
    assert!(managed_children(&managed).is_empty());
    assert!(!t.path().join("escape.txt").exists());
}

// ---------- defect 2: cleanup of imported files, with duplicates ----------
#[test]
fn purging_an_imported_workspace_removes_its_files_only_when_unreferenced() {
    let t = tempfile::tempdir().unwrap();
    let pkg = rich_package(t.path());
    let managed = t.path().join("managed");
    let mut s = Store::open_in_memory().unwrap();
    let a = import(&mut s, &pkg, &managed);
    let b = s.duplicate_workspace(&a.id, "Copy of Rich").unwrap();
    let a_dir = managed.join(&a.id);
    assert!(a_dir.exists());
    let none = HashSet::new();

    // Nothing is unreferenced yet.
    assert!(s.cleanup_managed_dirs(&managed, &none).unwrap().removed.is_empty());

    // Trashing keeps files (workspace is restorable).
    s.trash_workspace(&a.id).unwrap();
    assert!(s.cleanup_managed_dirs(&managed, &none).unwrap().removed.is_empty());
    assert!(a_dir.exists());

    // Purging the ORIGINAL must not delete files the DUPLICATE still uses.
    s.purge_workspace(&a.id).unwrap();
    let rep = s.cleanup_managed_dirs(&managed, &none).unwrap();
    assert!(rep.removed.is_empty(), "{rep:?}");
    assert!(a_dir.exists());
    assert!(
        s.check_sources(&b.id, true, &CancelToken::new())
            .unwrap()
            .iter()
            .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)),
        "duplicate must still have its files"
    );

    // Once the last user is gone, the folder is removed and space reclaimed.
    s.trash_workspace(&b.id).unwrap();
    s.purge_workspace(&b.id).unwrap();
    let rep = s.cleanup_managed_dirs(&managed, &none).unwrap();
    assert_eq!(rep.removed, vec![a_dir.clone()]);
    assert!(rep.bytes_freed > 0);
    assert!(!a_dir.exists());
}

#[test]
fn cleanup_is_conservative() {
    let t = tempfile::tempdir().unwrap();
    let managed = t.path().join("managed");
    let s = Store::open_in_memory().unwrap();
    let orphan = uuid::Uuid::new_v4().to_string();
    let protected_id = uuid::Uuid::new_v4().to_string();
    for d in [orphan.as_str(), protected_id.as_str(), "not-a-uuid", "UPPER-CASE-NOT-OURS"] {
        w(&managed, &format!("{d}/f.txt"), b"data");
    }
    w(&managed, "loose-file.txt", b"x");
    let protect: HashSet<String> = [protected_id.clone()].into_iter().collect();
    let rep = s.cleanup_managed_dirs(&managed, &protect).unwrap();
    assert_eq!(rep.removed, vec![managed.join(&orphan)], "{rep:?}");
    assert!(managed.join(&protected_id).join("f.txt").exists(), "in-progress imports are protected");
    assert!(
        managed.join("not-a-uuid/f.txt").exists()
            && managed.join("UPPER-CASE-NOT-OURS/f.txt").exists()
            && managed.join("loose-file.txt").exists()
    );
    // A missing managed root is not an error.
    assert!(s.cleanup_managed_dirs(&t.path().join("nope"), &protect).unwrap().removed.is_empty());
}

#[test]
fn cleanup_keeps_a_folder_referenced_by_a_relinked_or_differently_written_path() {
    let t = tempfile::tempdir().unwrap();
    let pkg = rich_package(t.path());
    let managed = t.path().join("managed");
    let mut s = Store::open_in_memory().unwrap();
    let a = import(&mut s, &pkg, &managed);
    s.trash_workspace(&a.id).unwrap();
    s.purge_workspace(&a.id).unwrap();
    // Another workspace references the folder through a differently written path (as Windows verbatim paths would).
    let other = s.create_workspace("Other", "", None).unwrap().id;
    let weird = format!("//?/C:/Data/imported/{}/dir/b.txt", a.id.to_uppercase());
    let scan = ScanReport {
        items: vec![ScannedItem {
            is_folder: false,
            dir: vec![],
            name: "b.txt".into(),
            source_path: Some(PathBuf::from(&weird)),
            size: 1,
            sha256: Some("h".into()),
            mime: None,
            mtime_ms: None,
        }],
        warnings: vec![],
    };
    s.add_scanned(&other, None, &scan).unwrap();
    let rep = s.cleanup_managed_dirs(&managed, &HashSet::new()).unwrap();
    assert!(rep.removed.is_empty(), "must keep anything any entry might still point into: {rep:?}");
    assert!(managed.join(&a.id).exists());
}

#[cfg(unix)]
#[test]
fn cleanup_never_follows_or_removes_symlinks() {
    let t = tempfile::tempdir().unwrap();
    let managed = t.path().join("managed");
    fs::create_dir_all(&managed).unwrap();
    let precious = w(t.path(), "precious/keep.txt", b"keep");
    let name = uuid::Uuid::new_v4().to_string();
    std::os::unix::fs::symlink(precious.parent().unwrap(), managed.join(&name)).unwrap();
    let s = Store::open_in_memory().unwrap();
    let rep = s.cleanup_managed_dirs(&managed, &HashSet::new()).unwrap();
    assert!(rep.removed.is_empty());
    assert!(precious.exists());
}

// ---------- add is atomic ----------
#[test]
fn a_failed_add_keeps_nothing_from_that_call() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("A", "", None).unwrap().id;
    let ok = ScannedItem {
        is_folder: false,
        dir: vec![],
        name: "good.txt".into(),
        source_path: Some(w(t.path(), "good.txt", b"g")),
        size: 1,
        sha256: Some("g".into()),
        mime: None,
        mtime_ms: None,
    };
    // The second item needs a folder whose name is invalid, which fails after the first insert.
    let bad = ScannedItem {
        is_folder: false,
        dir: vec!["bad/name".into()],
        name: "x.txt".into(),
        source_path: Some(w(t.path(), "x.txt", b"x")),
        size: 1,
        sha256: Some("x".into()),
        mime: None,
        mtime_ms: None,
    };
    let scan = ScanReport { items: vec![ok.clone(), bad], warnings: vec![] };
    assert!(s.add_scanned(&ws, None, &scan).is_err());
    assert!(s.list_entries(&ws).unwrap().is_empty(), "nothing from a failed add may remain");
    // And a good add afterwards still works.
    s.add_scanned(&ws, None, &ScanReport { items: vec![ok], warnings: vec![] }).unwrap();
    assert_eq!(s.list_entries(&ws).unwrap().len(), 1);
}

// ---------- opening packages from the command line ----------
#[test]
fn launch_arguments_accept_only_existing_parcel_files() {
    let t = tempfile::tempdir().unwrap();
    let good = w(t.path(), "My Pack.PARCEL", b"zip");
    let other = w(t.path(), "notes.txt", b"x");
    let dir_named = t.path().join("folder.parcel");
    fs::create_dir_all(&dir_named).unwrap();
    let rel = w(t.path(), "rel.parcel", b"z");
    let args = vec![
        "C:\\Program Files\\Parcel\\parcel.exe".to_string(), // program (skipped)
        "--flag".into(),
        "-x".into(),
        format!("\"{}\"", good.display()), // quoted, as Windows can pass it
        good.display().to_string(),        // duplicate
        other.display().to_string(),       // wrong extension
        dir_named.display().to_string(),   // directory
        t.path().join("missing.parcel").display().to_string(),
        "rel.parcel".into(), // relative to cwd
        String::new(),
    ];
    let got = package_paths_from_args(&args, Some(t.path()), true);
    assert_eq!(got, vec![good.clone(), t.path().join("rel.parcel")]);
    assert!(package_paths_from_args(["rel.parcel"], None, false).is_empty(), "relative paths without a cwd are ignored");
    let _ = rel;
}
