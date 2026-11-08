use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn parcel(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_parcel")).args(args).output().unwrap()
}
fn s(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn p(x: &Path) -> &str {
    x.to_str().unwrap()
}

#[test]
fn create_inspect_verify_extract_validate_compare_roundtrip() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("proj");
    fs::create_dir_all(src.join("sub")).unwrap();
    fs::write(src.join("a.txt"), "alpha").unwrap();
    fs::write(src.join("sub/b.txt"), "beta").unwrap();
    let pkg = t.path().join("p.parcel");
    let o = parcel(&["create", p(&pkg), p(&src), "--name", "CLI Test"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(parcel(&["create", p(&pkg), p(&src)]).status.code() == Some(1), "refuses overwrite");
    let o = parcel(&["inspect", p(&pkg)]);
    assert!(o.status.success() && s(&o).contains("CLI Test") && s(&o).contains("proj/sub/b.txt"));
    let o = parcel(&["inspect", p(&pkg), "--json"]);
    let v: serde_json::Value = serde_json::from_str(&s(&o)).unwrap();
    assert_eq!(v["format"], "parcel");
    assert!(parcel(&["verify", p(&pkg)]).status.success());
    assert!(s(&parcel(&["validate", p(&pkg)])).starts_with("OK"));
    let out = t.path().join("out");
    let o = parcel(&["extract", p(&pkg), p(&out), "--select", "proj/sub"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(fs::read(out.join("proj/sub/b.txt")).unwrap(), b"beta");
    assert!(!out.join("proj/a.txt").exists());
    // second extract into same dir fails by default, rename policy succeeds
    assert_eq!(parcel(&["extract", p(&pkg), p(&out)]).status.code(), Some(1));
    assert!(parcel(&["extract", p(&pkg), p(&out), "--on-conflict", "rename"]).status.success());
    // compare
    fs::write(src.join("c.txt"), "gamma").unwrap();
    let pkg2 = t.path().join("p2.parcel");
    assert!(parcel(&["create", p(&pkg2), p(&src), "--name", "CLI Test"]).status.success());
    let o = parcel(&["compare", p(&pkg), p(&pkg2)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(s(&o).contains("proj/c.txt"));
}

#[test]
fn verify_fails_with_nonzero_exit_on_tampering_and_usage_errors_exit_2() {
    let t = tempfile::tempdir().unwrap();
    let f = t.path().join("f.bin");
    let marker = b"CLI_TAMPER_MARKER_0123456789_ABCDEFGHIJ";
    fs::write(&f, marker).unwrap();
    let pkg = t.path().join("p.parcel");
    assert!(parcel(&["create", p(&pkg), p(&f), "--no-compress"]).status.success());
    let mut b = fs::read(&pkg).unwrap();
    let pos = b.windows(marker.len()).position(|w| w == marker).unwrap();
    b[pos + 5] ^= 1;
    fs::write(&pkg, b).unwrap();
    assert_eq!(parcel(&["verify", p(&pkg)]).status.code(), Some(1));
    assert_eq!(parcel(&["bogus"]).status.code(), Some(2));
    assert_eq!(parcel(&["verify"]).status.code(), Some(2));
}

#[test]
fn import_zip_and_invalid_manifest() {
    let t = tempfile::tempdir().unwrap();
    let z = t.path().join("a.zip");
    {
        let mut w = zip::ZipWriter::new(fs::File::create(&z).unwrap());
        w.start_file("x/y.txt", zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut w, b"zipped").unwrap();
        w.finish().unwrap();
    }
    let pkg = t.path().join("a.parcel");
    assert!(parcel(&["import-zip", p(&z), p(&pkg)]).status.success());
    assert!(parcel(&["verify", p(&pkg)]).status.success());
    let bad = t.path().join("m.json");
    fs::write(&bad, r#"{"format":"nope"}"#).unwrap();
    let o = parcel(&["validate", p(&bad)]);
    assert_eq!(o.status.code(), Some(1));
    assert!(s(&o).starts_with("INVALID"));
}

#[test]
fn closed_stdout_pipe_does_not_panic() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("d");
    fs::create_dir_all(&src).unwrap();
    for i in 0..200 {
        fs::write(src.join(format!("file-{i}.txt")), "x").unwrap();
    }
    let pkg = t.path().join("p.parcel");
    assert!(parcel(&["create", p(&pkg), p(&src)]).status.success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_parcel"))
        .args(["inspect", p(&pkg)])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // reader goes away immediately
    let out = child.wait_with_output().unwrap();
    assert_ne!(out.status.code(), Some(101), "must not panic: {}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
}
