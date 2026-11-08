//! The committed example packages must stay valid: this guards the spec, the examples and the engine against drift.
use parcel_core::cancel::CancelToken;
use parcel_core::limits::ResourceLimits;
use parcel_core::reader::PackageReader;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

#[test]
fn committed_examples_open_and_verify() {
    let mut n = 0;
    for e in std::fs::read_dir(dir()).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("parcel") {
            continue;
        }
        let r = PackageReader::open(&p, &ResourceLimits::default()).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let rep = r.verify(&CancelToken::new(), &|_| {}).unwrap();
        assert!(rep.ok && rep.unlisted.is_empty(), "{}: {}", p.display(), rep.to_text());
        n += 1;
    }
    assert!(n >= 2, "expected at least two example packages");
}

#[test]
fn research_example_has_the_documented_features() {
    let r = PackageReader::open(&dir().join("research-collection.parcel"), &ResourceLimits::default()).unwrap();
    let m = r.manifest();
    assert!(m.entries.iter().any(|e| e.path == "Sources/Papers/paper.md"));
    assert!(m.entries.iter().any(|e| e.kind == parcel_core::manifest::EntryKind::Link));
    assert!(m.entries.iter().any(|e| e.kind == parcel_core::manifest::EntryKind::Note));
    assert_eq!(m.dependencies.len(), 2);
    assert_eq!(m.references.len(), 1);
    assert!(parcel_core::intel::diagnose_dependencies(m).iter().all(|d| d.severity != parcel_core::intel::Severity::Error));
}
