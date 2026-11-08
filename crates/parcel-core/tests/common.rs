#![allow(dead_code)]
use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::manifest::*;
use parcel_core::writer::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

pub fn noop(_: &Progress) {}

pub fn write(dir: &Path, rel: &str, data: &[u8]) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, data).unwrap();
    p
}

pub fn sample_input(src: &Path) -> BuildInput {
    let a = write(src, "a/report.txt", b"quarterly report contents");
    let b = write(src, "b/image.png", b"\x89PNG\r\n\x1a\nnot really a png but sniffed as one");
    let c = write(src, "c/data.csv", b"x,y\n1,2\n3,4\n");
    let mut base = Manifest::new("Sample", "A sample package");
    base.metadata.insert("owner".into(), "tests".into());
    let mut items = vec![];
    items.push(BuildItem { entry: Entry::new(EntryKind::Folder, "f1", "Docs"), content: ItemContent::None });
    let mut e = Entry::new(EntryKind::File, "e1", "Docs/report.txt");
    e.description = "The report".into();
    e.tags = vec!["q3".into()];
    items.push(BuildItem { entry: e, content: ItemContent::File(a) });
    items.push(BuildItem { entry: Entry::new(EntryKind::File, "e2", "Docs/image.png"), content: ItemContent::File(b) });
    items.push(BuildItem { entry: Entry::new(EntryKind::File, "e3", "data.csv"), content: ItemContent::File(c) });
    items.push(BuildItem {
        entry: Entry::new(EntryKind::Note, "n1", "Notes/readme.md"),
        content: ItemContent::Bytes(b"# Hello\nmarkdown note".to_vec()),
    });
    let mut link = Entry::new(EntryKind::Link, "l1", "Links/rust");
    link.url = Some("https://www.rust-lang.org".into());
    items.push(BuildItem { entry: link, content: ItemContent::None });
    base.references.push(Reference {
        id: "r1".into(),
        title: "Rust".into(),
        url: "https://www.rust-lang.org".into(),
        description: String::new(),
        category: "docs".into(),
        entry_id: Some("e1".into()),
    });
    BuildInput { base, items, workspace_notes: "workspace notes".into(), thumbnail_png: None }
}

pub fn build(input: BuildInput, dest: &Path) -> parcel_core::Result<parcel_core::writer::BuildReport> {
    build_package(input, dest, ExportOptions::default(), &CancelToken::new(), &noop)
}

/// Hand-roll an archive (for hostile/malformed fixtures).
pub fn raw_zip(path: &Path, members: &[(&str, &[u8])]) {
    let f = fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    for (n, d) in members {
        z.start_file(*n, SimpleFileOptions::default()).unwrap();
        z.write_all(d).unwrap();
    }
    z.finish().unwrap();
}

pub fn read_zip_member(path: &Path, name: &str) -> Vec<u8> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut v = vec![];
    z.by_name(name).unwrap().read_to_end(&mut v).unwrap();
    v
}

/// Rewrite an existing package, letting the caller edit the manifest JSON.
pub fn rewrite_manifest(src: &Path, dst: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(fs::File::open(src).unwrap()).unwrap();
    let out = fs::File::create(dst).unwrap();
    let mut w = zip::ZipWriter::new(out);
    let mut edit = Some(edit);
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut data = vec![];
        f.read_to_end(&mut data).unwrap();
        if name == "manifest.json" {
            let mut v: serde_json::Value = serde_json::from_slice(&data).unwrap();
            (edit.take().unwrap())(&mut v);
            data = serde_json::to_vec(&v).unwrap();
        }
        w.start_file(name, SimpleFileOptions::default()).unwrap();
        w.write_all(&data).unwrap();
    }
    w.finish().unwrap();
}
