use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::hash::sha256_file;
use parcel_core::limits::ResourceLimits;
use parcel_core::reader::{ExtractOptions, PackageReader};
use parcel_workspace::collect::scan_paths;
use parcel_workspace::models::*;
use parcel_workspace::ops::ExportRequest;
use parcel_workspace::templates::ProfileSpec;
use parcel_workspace::{Store, WsError};
use std::fs;
use std::path::{Path, PathBuf};

fn noop(_: &Progress) {}
fn w(dir: &Path, rel: &str, data: &[u8]) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, data).unwrap();
    p
}
fn scan_add(s: &mut Store, ws: &str, parent: Option<&str>, paths: &[PathBuf]) -> parcel_workspace::ops::AddReport {
    let scan = scan_paths(paths, &CancelToken::new(), &noop).unwrap();
    s.add_scanned(ws, parent, &scan).unwrap()
}
fn tree(dir: &Path) -> Vec<(String, String)> {
    let mut out = vec![];
    fn walk(b: &Path, d: &Path, o: &mut Vec<(String, String)>) {
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(b, &p, o)
            } else {
                o.push((p.strip_prefix(b).unwrap().to_string_lossy().into(), sha256_file(&p).unwrap().0))
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}
fn q(s: &Store, ws: &str, text: &str) -> Vec<String> {
    names(&s.search(ws, &SearchQuery { text: text.into(), ..Default::default() }).unwrap())
}
fn names(v: &[Entry]) -> Vec<String> {
    v.iter().map(|e| e.name.clone()).collect()
}

#[test]
fn migrations_seed_and_persistence_across_reopen() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("parcel.db");
    let id;
    {
        let mut s = Store::open(&db).unwrap();
        assert_eq!(s.schema_version().unwrap(), 1);
        assert_eq!(s.list_templates().unwrap().iter().filter(|t| t.builtin).count(), 5);
        assert_eq!(s.list_profiles().unwrap().len(), 3);
        id = s.create_workspace("Persist me", "desc", None).unwrap().id;
        s.set_pref("theme", "dark").unwrap();
    }
    let s = Store::open(&db).unwrap();
    assert_eq!(s.get_workspace(&id).unwrap().name, "Persist me");
    assert_eq!(s.get_pref("theme").unwrap().as_deref(), Some("dark"));
    assert_eq!(s.list_templates().unwrap().len(), 5, "seeding must be idempotent");
}

#[test]
fn damaged_and_newer_databases_are_reported_not_clobbered() {
    let t = tempfile::tempdir().unwrap();
    let db = t.path().join("bad.db");
    fs::write(&db, b"this is definitely not an sqlite database, just text".repeat(50)).unwrap();
    let err = Store::open(&db).err().unwrap();
    assert!(matches!(err, WsError::Corrupt(_)), "{err:?}");
    assert!(fs::read(&db).unwrap().starts_with(b"this is definitely"), "file must be left untouched");
    let db2 = t.path().join("new.db");
    {
        let c = rusqlite::Connection::open(&db2).unwrap();
        c.pragma_update(None, "user_version", 99).unwrap();
    }
    assert!(matches!(Store::open(&db2).err().unwrap(), WsError::Invalid(_)));
}

#[test]
fn every_builtin_template_creates_a_functional_workspace() {
    let mut s = Store::open_in_memory().unwrap();
    for t in s.list_templates().unwrap() {
        let ws = s.create_workspace(&format!("From {}", t.name), "", Some(&t.id)).unwrap();
        let paths: Vec<String> = s.entry_paths(&ws.id).unwrap().into_values().collect();
        for f in &t.spec.folders {
            assert!(paths.contains(f), "{}: missing folder {f}", t.id);
        }
        for n in &t.spec.notes {
            assert!(paths.contains(&n.path), "{}: missing note {}", t.id, n.path);
        }
        for f in &t.spec.metadata_fields {
            assert!(ws.metadata.contains_key(&f.key));
        }
        assert!(!t.spec.folders.is_empty() && !t.spec.notes.is_empty());
    }
}

#[test]
fn workspace_lifecycle_rename_duplicate_trash_restore_purge() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("src");
    let f = w(&src, "keep.txt", b"original");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Alpha", "first", Some("tpl-documents")).unwrap();
    scan_add(&mut s, &ws.id, None, std::slice::from_ref(&f));
    s.add_reference(
        &ws.id,
        RefInput { title: "T".into(), url: "https://example.com".into(), description: String::new(), category: "c".into(), entry_id: None },
    )
    .unwrap();
    let renamed = s
        .update_workspace(&ws.id, WorkspaceUpdate { name: Some("Beta".into()), notes: Some("ws notes".into()), ..Default::default() })
        .unwrap();
    assert_eq!(renamed.name, "Beta");
    assert!(s.update_workspace(&ws.id, WorkspaceUpdate { name: Some("  ".into()), ..Default::default() }).is_err());
    let dup = s.duplicate_workspace(&ws.id, "Beta copy").unwrap();
    assert_ne!(dup.id, ws.id);
    assert_eq!(dup.notes, "ws notes");
    assert_eq!(s.list_entries(&dup.id).unwrap().len(), s.list_entries(&ws.id).unwrap().len());
    assert_eq!(s.list_references(&dup.id).unwrap().len(), 1);
    // Duplicated entries must not share ids with the original.
    let a: Vec<_> = s.list_entries(&ws.id).unwrap().into_iter().map(|e| e.id).collect();
    assert!(s.list_entries(&dup.id).unwrap().iter().all(|e| !a.contains(&e.id)));
    // Delete safely.
    assert!(s.purge_workspace(&ws.id).is_err(), "cannot purge a live workspace");
    s.trash_workspace(&ws.id).unwrap();
    assert!(s.list_workspaces(false).unwrap().iter().all(|x| x.id != ws.id));
    assert_eq!(s.list_workspaces(true).unwrap().len(), 1);
    s.restore_workspace(&ws.id).unwrap();
    assert_eq!(s.list_workspaces(false).unwrap().len(), 2);
    s.trash_workspace(&ws.id).unwrap();
    s.purge_workspace(&ws.id).unwrap();
    assert!(s.get_workspace(&ws.id).is_err());
    assert!(!s.list_entries(&dup.id).unwrap().is_empty(), "other workspace unaffected");
    assert_eq!(fs::read(&f).unwrap(), b"original", "source file untouched by workspace deletion");
}

#[test]
fn folders_rename_move_reorder_remove() {
    let t = tempfile::tempdir().unwrap();
    let a = w(t.path(), "a.txt", b"A");
    let b = w(t.path(), "b.txt", b"B");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("W", "", None).unwrap().id;
    let f1 = s.create_folder(&ws, None, "One").unwrap();
    let f2 = s.create_folder(&ws, Some(&f1.id), "Two").unwrap();
    assert!(matches!(s.create_folder(&ws, None, "one").err().unwrap(), WsError::NameConflict(_)), "case-insensitive");
    assert!(s.create_folder(&ws, None, "bad/name").is_err());
    assert!(s.create_folder(&ws, None, "..").is_err());
    scan_add(&mut s, &ws, Some(&f2.id), &[a.clone(), b.clone()]);
    let paths = s.entry_paths(&ws).unwrap();
    assert!(paths.values().any(|p| p == "One/Two/a.txt"));
    // cycle
    assert!(s.move_entries(std::slice::from_ref(&f1.id), Some(&f2.id)).is_err());
    assert!(s.move_entries(std::slice::from_ref(&f1.id), Some(&f1.id)).is_err());
    // move files to root, then conflict
    let files = s.list_children(&ws, Some(&f2.id)).unwrap();
    s.move_entries(&[files[0].id.clone()], None).unwrap();
    let again = w(t.path(), "other/a.txt", b"different");
    let scan = scan_paths(&[again], &CancelToken::new(), &noop).unwrap();
    s.add_scanned(&ws, None, &scan).unwrap(); // auto-renamed to "a (1).txt"
    let root = s.list_children(&ws, None).unwrap();
    assert!(names(&root).contains(&"a (1).txt".to_string()), "{:?}", names(&root));
    let moved_conflict = s.move_entries(std::slice::from_ref(&f1.id), None);
    assert!(moved_conflict.is_ok());
    // rename
    let e = s.rename_entry(&files[1].id, "renamed.txt").unwrap();
    assert_eq!(e.name, "renamed.txt");
    let dup = s.list_children(&ws, None).unwrap().into_iter().find(|e| e.name == "a (1).txt").unwrap();
    assert!(matches!(s.rename_entry(&dup.id, "A.TXT").err().unwrap(), WsError::NameConflict(_)));
    // reorder
    let ids: Vec<String> = s.list_children(&ws, None).unwrap().into_iter().map(|e| e.id).rev().collect();
    s.reorder(&ws, None, &ids).unwrap();
    // remove folder cascades inside workspace but never touches originals
    s.delete_entries(std::slice::from_ref(&f1.id)).unwrap();
    assert!(s.get_entry(&f2.id).is_err());
    assert!(a.exists() && b.exists());
}

#[test]
fn collection_from_multiple_locations_duplicates_and_symlinks() {
    let t = tempfile::tempdir().unwrap();
    let docs = t.path().join("docs");
    let pics = t.path().join("pics");
    w(&docs, "report.txt", b"same content");
    w(&docs, "sub/deep/n.md", b"# n");
    let copy = w(&pics, "copy-of-report.txt", b"same content");
    w(&pics, "cat.png", b"\x89PNG\r\n\x1a\nxx");
    #[cfg(unix)]
    std::os::unix::fs::symlink(docs.join("report.txt"), pics.join("link.txt")).unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Multi", "", None).unwrap().id;
    let rep = scan_add(&mut s, &ws, None, &[docs.clone(), pics.clone()]);
    assert_eq!(rep.added.len(), 4);
    assert_eq!(rep.content_duplicates.len(), 1);
    #[cfg(unix)]
    assert!(rep.warnings.iter().any(|w| w.contains("symbolic")));
    let paths: Vec<String> = s.entry_paths(&ws).unwrap().into_values().collect();
    assert!(paths.contains(&"docs/sub/deep/n.md".to_string()));
    assert!(paths.contains(&"pics/cat.png".to_string()));
    // Re-adding the same source is skipped, not duplicated.
    let again = scan_add(&mut s, &ws, None, std::slice::from_ref(&copy));
    assert_eq!(again.already_collected, vec!["copy-of-report.txt".to_string()]);
    let groups = s.find_duplicates(&ws).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].ids.len(), 2);
    assert_eq!(groups[0].wasted_bytes, 12);
    let png = s.list_entries(&ws).unwrap().into_iter().find(|e| e.name == "cat.png").unwrap();
    assert_eq!(png.mime.as_deref(), Some("image/png"));
}

#[test]
fn scan_can_be_cancelled() {
    let t = tempfile::tempdir().unwrap();
    for i in 0..20 {
        w(t.path(), &format!("f{i}.txt"), b"x");
    }
    let c = CancelToken::new();
    c.cancel();
    let err = scan_paths(&[t.path().to_path_buf()], &c, &noop).err().unwrap();
    assert!(matches!(err, WsError::Core(parcel_core::ParcelError::Cancelled)));
}

#[test]
fn source_missing_changed_relink_and_accept_flow() {
    let t = tempfile::tempdir().unwrap();
    let keep = w(t.path(), "keep.txt", b"keep me");
    let gone = w(t.path(), "gone.txt", b"will vanish");
    let edit = w(t.path(), "edit.txt", b"version one");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Src", "", None).unwrap().id;
    scan_add(&mut s, &ws, None, &[keep, gone.clone(), edit.clone()]);
    let st = |s: &Store| -> std::collections::HashMap<String, String> {
        s.check_sources(&ws, true, &CancelToken::new())
            .unwrap()
            .into_iter()
            .map(|c| (c.path, serde_json::to_value(&c.status).unwrap()["state"].as_str().unwrap().to_string()))
            .collect()
    };
    assert!(st(&s).values().all(|v| v == "unchanged"));
    let backup = fs::read(&gone).unwrap();
    fs::remove_file(&gone).unwrap();
    fs::write(&edit, b"version TWO!").unwrap();
    let m = st(&s);
    assert_eq!(m["gone.txt"], "missing");
    assert_eq!(m["edit.txt"], "changed");
    assert_eq!(m["keep.txt"], "unchanged");
    // Export is blocked and tells us exactly why.
    let req = ExportRequest { dest: t.path().join("o/x.parcel"), ..Default::default() };
    match s.export_workspace(&ws, &req, &CancelToken::new(), &noop).err().unwrap() {
        WsError::SourceProblems { missing, changed } => {
            assert_eq!(missing.len(), 1);
            assert_eq!(changed.len(), 1);
        }
        e => panic!("{e:?}"),
    }
    assert!(!req.dest.exists());
    // Relink missing to a different-content file requires explicit acceptance.
    let moved = w(t.path(), "elsewhere/gone-moved.txt", b"different bytes");
    let gone_id = s.list_entries(&ws).unwrap().into_iter().find(|e| e.name == "gone.txt").unwrap().id;
    assert!(s.relink_source(&gone_id, &moved, false).is_err());
    let right = w(t.path(), "elsewhere/restored.txt", &backup);
    assert!(s.relink_source(&gone_id, &right, false).unwrap().identical);
    // Accept the edit as the new baseline.
    let edit_id = s.list_entries(&ws).unwrap().into_iter().find(|e| e.name == "edit.txt").unwrap().id;
    assert_eq!(s.accept_source_changes(&[edit_id]).unwrap(), 1);
    assert!(st(&s).values().all(|v| v == "unchanged"));
    let out = s.export_workspace(&ws, &req, &CancelToken::new(), &noop).unwrap();
    assert_eq!(out.revision, 1);
}

#[test]
fn export_with_accept_changed_updates_baseline() {
    let t = tempfile::tempdir().unwrap();
    let f = w(t.path(), "doc.txt", b"before");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("A", "", None).unwrap().id;
    scan_add(&mut s, &ws, None, std::slice::from_ref(&f));
    fs::write(&f, b"after!!").unwrap();
    let req = ExportRequest { dest: t.path().join("a.parcel"), accept_changed: true, ..Default::default() };
    s.export_workspace(&ws, &req, &CancelToken::new(), &noop).unwrap();
    let r = PackageReader::open(&req.dest, &ResourceLimits::default()).unwrap();
    assert_eq!(r.manifest().entries[0].size, 7);
    assert!(s
        .check_sources(&ws, true, &CancelToken::new())
        .unwrap()
        .iter()
        .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)));
}

#[test]
fn notes_references_validation_and_search() {
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("N", "", None).unwrap().id;
    let n = s.create_note(&ws, None, "Plan", "# Plan\nbuy *milk*", None).unwrap();
    assert_eq!(n.name, "Plan.md");
    let n2 = s
        .update_entry(
            &n.id,
            EntryUpdate {
                content: Some("# Plan\nbuy oat drink".into()),
                tags: Some(vec!["todo".into(), " ".into()]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(n2.tags, vec!["todo"]);
    assert_eq!(n2.size, n2.content.as_ref().unwrap().len() as i64);
    let l = s.create_link(&ws, None, "Rust", "https://www.rust-lang.org", "lang").unwrap();
    assert!(s.create_link(&ws, None, "bad", "javascript:alert(1)", "").is_err());
    assert!(s.create_link(&ws, None, "bad2", "file:///etc/passwd", "").is_err());
    assert!(s
        .add_reference(
            &ws,
            RefInput { title: "x".into(), url: "ftp://ok".into(), description: String::new(), category: String::new(), entry_id: None }
        )
        .is_ok());
    assert!(s
        .add_reference(
            &ws,
            RefInput {
                title: "x".into(),
                url: "data:text/html,hi".into(),
                description: String::new(),
                category: String::new(),
                entry_id: None
            }
        )
        .is_err());
    assert!(s
        .add_reference(
            &ws,
            RefInput { title: " ".into(), url: "https://a.b".into(), description: String::new(), category: String::new(), entry_id: None }
        )
        .is_err());
    assert_eq!(q(&s, &ws, "oat"), vec!["Plan.md"], "searches note content");
    assert_eq!(q(&s, &ws, "kind:link"), vec!["Rust"]);
    assert_eq!(q(&s, &ws, "tag:todo"), vec!["Plan.md"]);
    assert_eq!(q(&s, &ws, "ext:md"), vec!["Plan.md"]);
    assert_eq!(q(&s, &ws, "lang"), vec!["Rust"], "searches descriptions");
    assert!(q(&s, &ws, "zzz").is_empty());
    assert_eq!(q(&s, &ws, "").len(), 2);
    s.batch_tag(&[n.id.clone(), l.id.clone()], &["shared".into()], &["todo".into()]).unwrap();
    assert_eq!(q(&s, &ws, "tag:shared").len(), 2);
    s.batch_set_metadata(std::slice::from_ref(&l.id), "owner", Some("me")).unwrap();
    assert_eq!(q(&s, &ws, "me").len(), 1);
    let sorted = s.search(&ws, &SearchQuery { sort: Some("name".into()), descending: true, ..Default::default() }).unwrap();
    assert_eq!(names(&sorted), vec!["Rust", "Plan.md"]);
    let st = s.workspace_stats(&ws).unwrap();
    assert_eq!((st.notes, st.links, st.files, st.references), (1, 1, 0, 1));
}

#[test]
fn templates_can_be_saved_from_workspaces_and_reused() {
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Base", "", Some("tpl-software")).unwrap().id;
    s.create_folder(&ws, None, "Custom").unwrap();
    let t = s.template_from_workspace(&ws, "My layout", "mine", true).unwrap();
    assert!(!t.builtin && t.spec.folders.contains(&"Custom".to_string()));
    let ws2 = s.create_workspace("Again", "", Some(&t.id)).unwrap();
    assert!(s.entry_paths(&ws2.id).unwrap().values().any(|p| p == "Custom"));
    assert!(s.delete_template("tpl-software").is_err());
    s.delete_template(&t.id).unwrap();
    // profiles
    assert!(s.save_profile(Some("prof-standard"), "x", "", &ProfileSpec::default()).is_err());
    let p = s.save_profile(None, "Mine", "", &ProfileSpec { compress: false, ..Default::default() }).unwrap();
    s.update_workspace(&ws, WorkspaceUpdate { export_profile_id: Some(Some(p.id.clone())), ..Default::default() }).unwrap();
    assert_eq!(s.get_workspace(&ws).unwrap().export_profile_id.as_deref(), Some(p.id.as_str()));
    s.delete_profile(&p.id).unwrap();
    assert_eq!(s.get_workspace(&ws).unwrap().export_profile_id, None);
}

#[test]
fn export_profile_exclusions_and_dependencies() {
    let t = tempfile::tempdir().unwrap();
    let a = w(t.path(), "a.txt", b"A");
    let junk = w(t.path(), "scratch.tmp", b"junk");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("P", "", None).unwrap().id;
    scan_add(&mut s, &ws, None, &[a, junk]);
    let es = s.list_entries(&ws).unwrap();
    let (a_id, j_id) =
        (es.iter().find(|e| e.name == "a.txt").unwrap().id.clone(), es.iter().find(|e| e.name == "scratch.tmp").unwrap().id.clone());
    s.add_dependency(&ws, &a_id, "entry", &j_id, "needs junk", true).unwrap();
    s.add_dependency(&ws, &a_id, "external", "Python 3.12", "", false).unwrap();
    assert!(s.add_dependency(&ws, &a_id, "bogus", "x", "", true).is_err());
    s.update_workspace(&ws, WorkspaceUpdate { export_profile_id: Some(Some("prof-clean".into())), ..Default::default() }).unwrap();
    let req = ExportRequest { dest: t.path().join("p.parcel"), ..Default::default() };
    let out = s.export_workspace(&ws, &req, &CancelToken::new(), &noop).unwrap();
    assert_eq!(out.excluded, 1);
    let r = PackageReader::open(&req.dest, &ResourceLimits::default()).unwrap();
    assert_eq!(r.manifest().entries.len(), 1);
    assert_eq!(r.manifest().dependencies.len(), 1, "dependency on excluded entry is dropped");
    assert!(parcel_core::intel::diagnose_dependencies(r.manifest()).iter().all(|d| d.severity != parcel_core::intel::Severity::Error));
}

/// The mandatory end-to-end workflow from the product requirements.
#[test]
fn full_workflow_create_collect_organize_export_reopen_verify_extract() {
    let t = tempfile::tempdir().unwrap();
    let loc1 = t.path().join("Documents");
    let loc2 = t.path().join("Downloads");
    let f1 = w(&loc1, "thesis.txt", b"Chapter one.\nChapter two.\n");
    let f2 = w(&loc2, "figure1.png", &[0x89, b'P', b'N', b'G', 13, 10, 26, 10, 1, 2, 3, 4, 5]);
    let f3 = w(&loc2, "data/measurements.csv", b"t,v\n0,1\n1,2\n");
    let before = tree(t.path());
    let db = t.path().join("app/parcel.db");
    let pkg = t.path().join("out/Thesis.parcel");
    let note_id;
    {
        // 1-2 create workspace and add files from multiple locations
        let mut s = Store::open(&db).unwrap();
        let ws = s.create_workspace("Thesis", "My thesis materials", Some("tpl-research")).unwrap();
        let papers = s.ensure_folder_path(&ws.id, "Sources/Papers").unwrap();
        let data = s.ensure_folder_path(&ws.id, "Data").unwrap();
        scan_add(&mut s, &ws.id, Some(&papers), std::slice::from_ref(&f1));
        scan_add(&mut s, &ws.id, None, std::slice::from_ref(&f2));
        scan_add(&mut s, &ws.id, Some(&data), std::slice::from_ref(&f3));
        // 3 organise
        let fig = s.list_entries(&ws.id).unwrap().into_iter().find(|e| e.name == "figure1.png").unwrap();
        let figs = s.ensure_folder_path(&ws.id, "Figures").unwrap();
        s.move_entries(std::slice::from_ref(&fig.id), Some(&figs)).unwrap();
        s.update_entry(
            &fig.id,
            EntryUpdate { description: Some("Main figure".into()), tags: Some(vec!["figure".into()]), ..Default::default() },
        )
        .unwrap();
        // 4 notes and references
        let n = s.create_note(&ws.id, None, "Summary", "# Summary\nAll **good**.", Some(&fig.id)).unwrap();
        note_id = n.id.clone();
        s.add_reference(
            &ws.id,
            RefInput {
                title: "Rust book".into(),
                url: "https://doc.rust-lang.org/book/".into(),
                description: "read".into(),
                category: "docs".into(),
                entry_id: None,
            },
        )
        .unwrap();
        s.update_workspace(&ws.id, WorkspaceUpdate { notes: Some("Overall workspace notes".into()), ..Default::default() }).unwrap();
        // 5 export
        let out =
            s.export_workspace(&ws.id, &ExportRequest { dest: pkg.clone(), ..Default::default() }, &CancelToken::new(), &noop).unwrap();
        assert_eq!(out.revision, 1);
        assert_eq!(s.list_revisions(&ws.id).unwrap().len(), 1);
        assert_eq!(s.list_recent_packages(10).unwrap().len(), 1);
        assert!(s.list_activity(None, 100).unwrap().iter().any(|a| a.kind == "package.exported"));
        // 6 "close application" – store dropped here
    }
    // 7 reopen package without any database
    let mut r = PackageReader::open(&pkg, &ResourceLimits::default()).unwrap();
    assert_eq!(r.manifest().name, "Thesis");
    assert_eq!(r.notes().unwrap(), "Overall workspace notes");
    assert_eq!(r.manifest().references[0].title, "Rust book");
    // no absolute source paths in the package
    let raw = serde_json::to_string(r.manifest()).unwrap();
    assert!(!raw.contains(t.path().to_str().unwrap()), "manifest leaked a local path");
    // 8 verify integrity
    let rep = r.verify(&CancelToken::new(), &noop).unwrap();
    assert!(rep.ok, "{}", rep.to_text());
    // 9 extract selected content
    let sel: Vec<String> = r.manifest().entries.iter().filter(|e| e.path == "Figures" || e.path == "Data").map(|e| e.id.clone()).collect();
    assert_eq!(sel.len(), 2);
    let out_dir = t.path().join("extracted");
    r.extract(&out_dir, &ExtractOptions { selection: Some(sel), ..Default::default() }, &CancelToken::new(), &noop).unwrap();
    // 10 extracted == originals
    assert_eq!(fs::read(out_dir.join("Figures/figure1.png")).unwrap(), fs::read(&f2).unwrap());
    assert_eq!(fs::read(out_dir.join("Data/measurements.csv")).unwrap(), fs::read(&f3).unwrap());
    assert!(!out_dir.join("Sources").exists(), "unselected content must not be extracted");
    // sources unchanged (only new output files appeared)
    let after: Vec<_> =
        tree(t.path()).into_iter().filter(|(p, _)| !p.starts_with("app") && !p.starts_with("out") && !p.starts_with("extracted")).collect();
    let before: Vec<_> = before.into_iter().filter(|(p, _)| !p.starts_with("app")).collect();
    assert_eq!(before, after, "source files must be byte-identical after the workflow");
    // The attached-note association survives in the manifest.
    let note = r.manifest().entries.iter().find(|e| e.path == "Summary.md").unwrap();
    assert!(note.metadata.contains_key("parcel.attached_to"));
    let _ = note_id;
}

#[test]
fn import_package_into_workspace_then_reexport_as_next_revision() {
    let t = tempfile::tempdir().unwrap();
    let f = w(t.path(), "src/a.txt", b"alpha");
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("Round", "d", None).unwrap().id;
    scan_add(&mut s, &ws, None, std::slice::from_ref(&f));
    s.create_note(&ws, None, "n", "note body", None).unwrap();
    s.create_link(&ws, None, "site", "https://example.com", "").unwrap();
    s.add_reference(
        &ws,
        RefInput {
            title: "r".into(),
            url: "https://example.com/r".into(),
            description: String::new(),
            category: "x".into(),
            entry_id: None,
        },
    )
    .unwrap();
    let p1 = t.path().join("r1.parcel");
    s.export_workspace(&ws, &ExportRequest { dest: p1.clone(), ..Default::default() }, &CancelToken::new(), &noop).unwrap();
    // Import elsewhere.
    let mut s2 = Store::open_in_memory().unwrap();
    let managed = t.path().join("managed");
    let imp = s2.import_package(&p1, &managed, &ResourceLimits::default(), &CancelToken::new(), &noop).unwrap();
    assert_eq!(imp.name, "Round");
    assert_eq!(imp.revision, 1);
    assert_eq!(s2.list_entries(&imp.id).unwrap().len(), 3);
    let imported = s2.list_entries(&imp.id).unwrap().into_iter().find(|e| e.kind == "file").unwrap();
    assert_eq!(imported.origin, "imported");
    assert_eq!(fs::read(imported.source_path.as_ref().unwrap()).unwrap(), b"alpha");
    assert!(s2
        .check_sources(&imp.id, true, &CancelToken::new())
        .unwrap()
        .iter()
        .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)));
    // Modify and export as revision 2 in the same lineage.
    s2.create_note(&imp.id, None, "extra", "more", None).unwrap();
    let p2 = t.path().join("r2.parcel");
    let out = s2.export_workspace(&imp.id, &ExportRequest { dest: p2.clone(), ..Default::default() }, &CancelToken::new(), &noop).unwrap();
    assert_eq!(out.revision, 2);
    let (m1, m2) = (
        PackageReader::open(&p1, &Default::default()).unwrap().manifest().clone(),
        PackageReader::open(&p2, &Default::default()).unwrap().manifest().clone(),
    );
    assert_eq!(m1.package_id, m2.package_id, "same lineage");
    let c = parcel_core::compare::compare_manifests(&m1, &m2);
    assert_eq!(c.added.len(), 1);
    assert_eq!(c.unchanged.len(), 3);
    // A failed import must leave nothing behind.
    let bad = t.path().join("bad.parcel");
    fs::write(&bad, b"garbage").unwrap();
    let before = s2.list_workspaces(false).unwrap().len();
    assert!(s2.import_package(&bad, &managed, &ResourceLimits::default(), &CancelToken::new(), &noop).is_err());
    assert_eq!(s2.list_workspaces(false).unwrap().len(), before);
}

#[test]
fn zip_import_then_export_as_parcel_workflow() {
    let t = tempfile::tempdir().unwrap();
    let zp = t.path().join("legacy.zip");
    {
        let f = fs::File::create(&zp).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (n, d) in [("proj/readme.txt", &b"hello zip"[..]), ("proj/data/x.csv", b"a,b\n1,2\n")] {
            z.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut z, d).unwrap();
        }
        z.finish().unwrap();
    }
    let tmp_pkg = t.path().join("converted.parcel");
    parcel_core::zipimport::convert_zip_to_parcel(&zp, &tmp_pkg, "Legacy", Default::default(), &CancelToken::new(), &noop).unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let imp = s.import_package(&tmp_pkg, &t.path().join("managed"), &ResourceLimits::default(), &CancelToken::new(), &noop).unwrap();
    let out_pkg = t.path().join("final.parcel");
    s.export_workspace(&imp.id, &ExportRequest { dest: out_pkg.clone(), ..Default::default() }, &CancelToken::new(), &noop).unwrap();
    let mut r = PackageReader::open(&out_pkg, &ResourceLimits::default()).unwrap();
    assert!(r.verify(&CancelToken::new(), &noop).unwrap().ok);
    let x = t.path().join("x");
    r.extract(&x, &ExtractOptions::default(), &CancelToken::new(), &noop).unwrap();
    assert_eq!(fs::read(x.join("proj/readme.txt")).unwrap(), b"hello zip");
    assert_eq!(fs::read(x.join("proj/data/x.csv")).unwrap(), b"a,b\n1,2\n");
}

#[test]
fn glob_matching() {
    use parcel_workspace::templates::glob_match as g;
    assert!(g("*.tmp", "a.TMP") && g("~$*", "~$doc.docx") && g("thumbs.db", "Thumbs.db") && g("a?c", "abc"));
    assert!(!g("*.tmp", "a.txt") && !g("a?c", "ac") && g("*", "") && !g("?", ""));
}
