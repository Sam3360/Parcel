mod common;
use common::*;
use parcel_core::compare::compare_manifests;
use parcel_core::intel::*;
use parcel_core::manifest::*;
use std::fs;

#[test]
fn duplicates_grouped_by_hash() {
    let items = vec![
        HashedItem { id: "1".into(), label: "a".into(), size: 10, sha256: "h1".into() },
        HashedItem { id: "2".into(), label: "b".into(), size: 10, sha256: "h1".into() },
        HashedItem { id: "3".into(), label: "c".into(), size: 10, sha256: "h1".into() },
        HashedItem { id: "4".into(), label: "d".into(), size: 5, sha256: "h2".into() },
        HashedItem { id: "5".into(), label: "e".into(), size: 0, sha256: "empty".into() },
        HashedItem { id: "6".into(), label: "f".into(), size: 0, sha256: "empty".into() },
    ];
    let g = find_duplicates(&items);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].ids, vec!["1", "2", "3"]);
    assert_eq!(g[0].wasted_bytes, 20);
}

#[test]
fn source_status_unchanged_changed_missing() {
    let t = tempfile::tempdir().unwrap();
    let p = write(t.path(), "f.txt", b"original");
    let (sha, size) = parcel_core::hash::sha256_file(&p).unwrap();
    let rec = SourceRecord { id: "x".into(), path: p.clone(), size, sha256: sha, modified_ms: mtime_ms(&p) };
    assert_eq!(check_source(&rec, false), SourceStatus::Unchanged);
    assert_eq!(check_source(&rec, true), SourceStatus::Unchanged);
    // Same size, different content, forged-equal mtime is only caught by the deep check.
    fs::write(&p, b"0riginal").unwrap();
    assert!(matches!(check_source(&rec, true), SourceStatus::Changed { .. }));
    // Touched but identical content → still unchanged.
    fs::write(&p, b"original").unwrap();
    let mut touched = rec.clone();
    touched.modified_ms = Some(1);
    assert_eq!(check_source(&touched, false), SourceStatus::Unchanged);
    // Size change.
    fs::write(&p, b"longer than before").unwrap();
    assert!(matches!(check_source(&rec, false), SourceStatus::Changed { .. }));
    fs::remove_file(&p).unwrap();
    assert_eq!(check_source(&rec, false), SourceStatus::Missing);
}

#[test]
fn replacement_validation() {
    let t = tempfile::tempdir().unwrap();
    let p = write(t.path(), "new.txt", b"hello");
    let sha = parcel_core::hash::sha256_bytes(b"hello");
    assert!(validate_replacement(&p, &sha).unwrap().identical);
    assert!(!validate_replacement(&p, &"0".repeat(64)).unwrap().identical);
    assert!(validate_replacement(t.path(), &sha).is_err());
}

#[test]
fn dependency_diagnostics() {
    let mut m = Manifest::new("D", "");
    for (id, p) in [("a", "a.txt"), ("b", "b.txt")] {
        let mut e = Entry::new(EntryKind::File, id, p);
        e.storage_path = Some(format!("content/{p}"));
        e.sha256 = Some("0".repeat(64));
        m.entries.push(e);
    }
    let dep = |id: &str, from: &str, to: &str, k: DependencyKind| Dependency {
        id: id.into(),
        kind: k,
        from_entry: from.into(),
        target: to.into(),
        description: String::new(),
        required: true,
    };
    m.dependencies = vec![
        dep("d1", "a", "b", DependencyKind::Entry),
        dep("d2", "b", "a", DependencyKind::Entry),
        dep("d3", "a", "ghost", DependencyKind::Entry),
        dep("d4", "a", "Python 3.11", DependencyKind::External),
    ];
    let d = diagnose_dependencies(&m);
    let codes: Vec<_> = d.iter().map(|x| x.code.as_str()).collect();
    assert!(codes.contains(&"dependency.cycle"));
    assert!(codes.contains(&"dependency.missing_target"));
    assert!(codes.contains(&"dependency.external"));
}

#[test]
fn comparison_reports_added_removed_changed_moved_unchanged_metadata() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a.parcel");
    build(sample_input(&t.path().join("s")), &a).unwrap();
    let old = parcel_core::reader::PackageReader::open(&a, &Default::default()).unwrap().manifest().clone();
    let mut new = old.clone();
    // changed content
    new.entries.iter_mut().find(|e| e.path == "data.csv").unwrap().sha256 = Some("f".repeat(64));
    // moved
    new.entries.iter_mut().find(|e| e.path == "Docs/image.png").unwrap().path = "Docs/moved.png".into();
    // metadata-only change
    new.entries.iter_mut().find(|e| e.path == "Docs/report.txt").unwrap().description = "edited".into();
    // removed + added
    new.entries.retain(|e| e.path != "Links/rust");
    let mut extra = Entry::new(EntryKind::File, "z", "new.txt");
    extra.sha256 = Some("1".repeat(64));
    extra.size = 3;
    extra.storage_path = Some("content/new.txt".into());
    new.entries.push(extra);
    new.name = "Renamed".into();
    let c = compare_manifests(&old, &new);
    assert_eq!(c.changed.iter().map(|d| d.path.as_str()).collect::<Vec<_>>(), vec!["data.csv"]);
    assert_eq!(c.moved.len(), 1);
    assert!(c.moved[0].detail[0].contains("Docs/image.png"));
    assert_eq!(c.metadata_changed.len(), 1);
    assert_eq!(c.added.len(), 1);
    assert_eq!(c.removed.len(), 1);
    assert!(c.unchanged.contains(&"Docs".to_string()));
    assert!(c.package_differences.iter().any(|d| d.contains("Renamed")));
    assert!(!c.is_identical());
    assert!(compare_manifests(&old, &old).is_identical());
}
