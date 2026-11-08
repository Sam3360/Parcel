//! The published JSON Schema must accept what the engine writes and reject what the engine rejects.
mod common;
use common::*;
use parcel_core::reader::PackageReader;
use serde_json::{json, Value};

fn schema() -> jsonschema::Validator {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../format/schemas/manifest.schema.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    jsonschema::validator_for(&v).expect("schema compiles")
}

fn written_manifest() -> Value {
    let t = tempfile::tempdir().unwrap();
    let dest = t.path().join("p.parcel");
    build(sample_input(&t.path().join("s")), &dest).unwrap();
    let r = PackageReader::open(&dest, &Default::default()).unwrap();
    serde_json::to_value(r.manifest()).unwrap()
}

#[test]
fn engine_output_conforms_to_schema() {
    let v = written_manifest();
    let s = schema();
    let errs: Vec<String> = s.iter_errors(&v).map(|e| e.to_string()).collect();
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn schema_rejects_the_same_things_the_engine_rejects() {
    let s = schema();
    let base = written_manifest();
    type Edit = Box<dyn Fn(&mut Value)>;
    let cases: Vec<(&str, Edit)> = vec![
        ("wrong format", Box::new(|v| v["format"] = json!("zip"))),
        ("bad version", Box::new(|v| v["format_version"] = json!("one"))),
        ("bad uuid-less name", Box::new(|v| v["name"] = json!(""))),
        ("traversal path", Box::new(|v| v["entries"][1]["path"] = json!("../x"))),
        ("absolute path", Box::new(|v| v["entries"][1]["path"] = json!("/etc/passwd"))),
        ("backslash path", Box::new(|v| v["entries"][1]["path"] = json!("a\\b"))),
        ("drive path", Box::new(|v| v["entries"][1]["path"] = json!("C:/x"))),
        ("short sha", Box::new(|v| v["entries"][1]["sha256"] = json!("abc"))),
        ("storage outside content", Box::new(|v| v["entries"][1]["storage_path"] = json!("manifest.json"))),
        (
            "file without sha",
            Box::new(|v| {
                v["entries"][1].as_object_mut().unwrap().remove("sha256");
            }),
        ),
        ("folder with sha", Box::new(|v| v["entries"][0]["sha256"] = json!("a".repeat(64)))),
        ("javascript url", Box::new(|v| v["references"][0]["url"] = json!("javascript:alert(1)"))),
        ("file url on link", Box::new(|v| v["entries"][5]["url"] = json!("file:///C:/Windows"))),
        (
            "link without url",
            Box::new(|v| {
                v["entries"][5].as_object_mut().unwrap().remove("url");
            }),
        ),
        ("bad kind", Box::new(|v| v["entries"][1]["kind"] = json!("symlink"))),
    ];
    for (label, edit) in cases {
        let mut v = base.clone();
        edit(&mut v);
        assert!(!s.is_valid(&v), "schema wrongly accepted: {label}");
    }
}
