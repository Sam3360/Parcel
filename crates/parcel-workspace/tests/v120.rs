//! 1.2.0: undoable removal and cleanup of Parcel's stored copies of imported files.
use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::limits::ResourceLimits;
use parcel_workspace::collect::scan_paths;
use parcel_workspace::models::{EntryUpdate, RefInput};
use parcel_workspace::ops::ExportRequest;
use parcel_workspace::removal::MAX_UNDO;
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
fn add(s: &mut Store, ws: &str, parent: Option<&str>, paths: &[PathBuf]) {
    let scan = scan_paths(paths, &CancelToken::new(), &noop).unwrap();
    s.add_scanned(ws, parent, &scan).unwrap();
}
fn id_of(s: &Store, ws: &str, name: &str) -> String {
    s.list_entries(ws).unwrap().into_iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no entry {name}")).id
}
fn names(s: &Store, ws: &str) -> Vec<String> {
    let mut v: Vec<String> = s.list_entries(ws).unwrap().into_iter().map(|e| e.name).collect();
    v.sort();
    v
}
fn json<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap()
}

/// A store backed by a file (so a second connection can tamper with rows) and a managed folder.
struct Env {
    t: tempfile::TempDir,
    s: Store,
    managed: PathBuf,
}
fn env() -> Env {
    let t = tempfile::tempdir().unwrap();
    let managed = t.path().join("managed");
    fs::create_dir_all(&managed).unwrap();
    let s = Store::open(&t.path().join("db/parcel.db")).unwrap();
    s.set_managed_root(Some(managed.clone()));
    Env { t, s, managed }
}
impl Env {
    /// Import a package with `n` files named f00.txt.. at the root plus a folder `Dir` with two files; returns the workspace id.
    fn imported(&mut self, n: usize) -> String {
        let mut src = Store::open_in_memory().unwrap();
        let ws = src.create_workspace("Src", "", None).unwrap().id;
        let base = self.t.path().join(format!("src-{n}"));
        let mut paths: Vec<PathBuf> = (0..n).map(|i| w(&base, &format!("f{i:02}.txt"), format!("content {i}").as_bytes())).collect();
        w(&base, "Dir/one.txt", b"one");
        w(&base, "Dir/Deep/two.txt", b"two");
        paths.push(base.join("Dir"));
        add(&mut src, &ws, None, &paths);
        let pkg = self.t.path().join(format!("pkg-{n}.parcel"));
        src.export_workspace(&ws, &ExportRequest { dest: pkg.clone(), overwrite: true, ..Default::default() }, &CancelToken::new(), &noop)
            .unwrap();
        self.s.import_package(&pkg, &self.managed, &ResourceLimits::default(), &CancelToken::new(), &noop).unwrap().id
    }
    fn path_of(&self, ws: &str, name: &str) -> PathBuf {
        PathBuf::from(self.s.list_entries(ws).unwrap().into_iter().find(|e| e.name == name).unwrap().source_path.unwrap())
    }
}

// ---------------- undo ----------------
#[test]
fn removing_then_undoing_restores_every_row_exactly() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("U", "", None).unwrap().id;
    let a = w(t.path(), "P/a.txt", b"A");
    let b = w(t.path(), "b.txt", b"B");
    add(&mut s, &ws, None, &[t.path().join("P"), b]);
    let _ = a;
    let (p, ia, ib) = (id_of(&s, &ws, "P"), id_of(&s, &ws, "a.txt"), id_of(&s, &ws, "b.txt"));
    s.update_entry(
        &ia,
        EntryUpdate {
            description: Some("desc".into()),
            tags: Some(vec!["t1".into()]),
            metadata: Some([("k".to_string(), "v".to_string())].into()),
            ..Default::default()
        },
    )
    .unwrap();
    let sub = s.create_folder(&ws, Some(&p), "Sub").unwrap();
    s.create_note(&ws, Some(&sub.id), "inner", "x", None).unwrap();
    let note = s.create_note(&ws, None, "n", "note text", Some(&ia)).unwrap();
    s.add_reference(
        &ws,
        RefInput {
            title: "r".into(),
            url: "https://example.com".into(),
            description: String::new(),
            category: "c".into(),
            entry_id: Some(ia.clone()),
        },
    )
    .unwrap();
    s.add_dependency(&ws, &ia, "entry", &ib, "a needs b", true).unwrap();
    s.add_dependency(&ws, &ib, "entry", &ia, "b needs a", true).unwrap();
    s.add_dependency(&ws, &ia, "external", "Python", "", false).unwrap();

    let sort = |mut v: Vec<serde_json::Value>| {
        v.sort_by_key(|x| x["id"].as_str().unwrap().to_string());
        v
    };
    let before = (
        sort(s.list_entries(&ws).unwrap().iter().map(json).collect()),
        sort(s.list_references(&ws).unwrap().iter().map(json).collect()),
        sort(s.list_dependencies(&ws).unwrap().iter().map(json).collect()),
    );

    let rec = s.remove_entries(std::slice::from_ref(&p)).unwrap();
    assert_eq!(rec.removed, 4, "P, a.txt, Sub and inner.md: {rec:?}");
    assert_eq!(names(&s, &ws), vec!["b.txt", "n.md"]);
    assert!(s.list_references(&ws).unwrap().iter().all(|r| r.entry_id.is_none()), "reference loses its link while removed");
    assert_eq!(s.get_entry(&note.id).unwrap().attached_to, None);
    assert_eq!(s.list_dependencies(&ws).unwrap().len(), 1, "only b→a remains (dangling) while a is removed");
    assert_eq!(s.list_removals(&ws).len(), 1);

    let out = s.undo_remove(&ws).unwrap();
    assert_eq!((out.restored, out.renamed, out.relocated), (4, 0, 0));
    let after = (
        sort(s.list_entries(&ws).unwrap().iter().map(json).collect()),
        sort(s.list_references(&ws).unwrap().iter().map(json).collect()),
        sort(s.list_dependencies(&ws).unwrap().iter().map(json).collect()),
    );
    assert_eq!(before.0, after.0, "entries (ids, order, tags, metadata, timestamps, attachments) must be identical");
    assert_eq!(before.1, after.1, "references must be identical");
    assert_eq!(before.2, after.2, "dependencies must be identical");
    assert!(s.list_removals(&ws).is_empty());
    assert!(s.undo_remove(&ws).is_err(), "nothing left to undo");
}

#[test]
fn undo_is_last_in_first_out() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("U", "", None).unwrap().id;
    add(&mut s, &ws, None, &[w(t.path(), "1.txt", b"1"), w(t.path(), "2.txt", b"2"), w(t.path(), "3.txt", b"3")]);
    for n in ["1.txt", "2.txt"] {
        s.remove_entries(&[id_of(&s, &ws, n)]).unwrap();
    }
    assert_eq!(s.list_removals(&ws).iter().map(|r| r.label.clone()).collect::<Vec<_>>(), vec!["2.txt", "1.txt"]);
    s.undo_remove(&ws).unwrap();
    assert_eq!(names(&s, &ws), vec!["2.txt", "3.txt"]);
    s.undo_remove(&ws).unwrap();
    assert_eq!(names(&s, &ws), vec!["1.txt", "2.txt", "3.txt"]);
}

#[test]
fn undo_puts_items_at_the_root_when_their_folder_is_gone() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("U", "", None).unwrap().id;
    add(&mut s, &ws, None, &[t.path().join("P")]);
    w(t.path(), "P/x.txt", b"x");
    add(&mut s, &ws, None, &[t.path().join("P")]);
    let (p, x) = (id_of(&s, &ws, "P"), id_of(&s, &ws, "x.txt"));
    s.remove_entries(&[x]).unwrap();
    s.delete_entries(&[p]).unwrap(); // the folder disappears by a non-undoable path
    let out = s.undo_remove(&ws).unwrap();
    assert_eq!((out.restored, out.relocated), (1, 1));
    let e = s.list_entries(&ws).unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].parent_id, None);
}

#[test]
fn undo_renames_instead_of_overwriting_when_the_name_was_taken() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("U", "", None).unwrap().id;
    add(&mut s, &ws, None, &[w(t.path(), "a/a.txt", b"old")]);
    s.remove_entries(&[id_of(&s, &ws, "a.txt")]).unwrap();
    add(&mut s, &ws, None, &[w(t.path(), "b/a.txt", b"new")]); // a different file with the same name
    let out = s.undo_remove(&ws).unwrap();
    assert_eq!((out.restored, out.renamed), (1, 1));
    assert_eq!(names(&s, &ws), vec!["a (1).txt", "a.txt"]);
}

#[test]
fn undo_refuses_for_a_trashed_workspace_and_keeps_the_history() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("U", "", None).unwrap().id;
    add(&mut s, &ws, None, &[w(t.path(), "a.txt", b"a")]);
    s.remove_entries(&[id_of(&s, &ws, "a.txt")]).unwrap();
    s.trash_workspace(&ws).unwrap();
    assert!(matches!(s.undo_remove(&ws).err().unwrap(), WsError::Invalid(_)));
    assert_eq!(s.list_removals(&ws).len(), 1);
    s.restore_workspace(&ws).unwrap();
    s.undo_remove(&ws).unwrap();
    assert_eq!(names(&s, &ws), vec!["a.txt"]);
}

#[test]
fn purging_a_workspace_discards_its_undo_history() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("U", "", None).unwrap().id;
    add(&mut s, &ws, None, &[w(t.path(), "a.txt", b"a")]);
    s.remove_entries(&[id_of(&s, &ws, "a.txt")]).unwrap();
    s.trash_workspace(&ws).unwrap();
    s.purge_workspace(&ws).unwrap();
    assert!(s.list_removals(&ws).is_empty());
}

#[test]
fn a_bad_removal_request_changes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let mut s = Store::open_in_memory().unwrap();
    let ws = s.create_workspace("A", "", None).unwrap().id;
    let other = s.create_workspace("B", "", None).unwrap().id;
    add(&mut s, &ws, None, &[w(t.path(), "a.txt", b"a")]);
    add(&mut s, &other, None, &[w(t.path(), "o.txt", b"o")]);
    let a = id_of(&s, &ws, "a.txt");
    assert!(matches!(s.remove_entries(&[a.clone(), "ghost".into()]).err().unwrap(), WsError::NotFound(_)));
    assert!(s.remove_entries(&[a.clone(), id_of(&s, &other, "o.txt")]).is_err(), "cross-workspace removal is refused");
    assert!(s.remove_entries(&[]).is_err());
    assert_eq!(names(&s, &ws), vec!["a.txt"]);
    assert_eq!(names(&s, &other), vec!["o.txt"]);
    assert!(s.list_removals(&ws).is_empty());
}

// ---------------- stored-file cleanup ----------------
#[test]
fn a_stored_copy_survives_until_the_removal_is_final_then_goes() {
    let mut e = env();
    let ws = e.imported(3);
    let f = e.path_of(&ws, "f01.txt");
    let other = e.path_of(&ws, "f02.txt");
    assert!(f.exists());
    let rec = e.s.remove_entries(&[id_of(&e.s, &ws, "f01.txt")]).unwrap();
    assert_eq!(rec.stored_files, 1);
    assert!(f.exists(), "undo must still be possible, so the stored copy is kept");
    e.s.undo_remove(&ws).unwrap();
    assert!(f.exists());
    assert!(e
        .s
        .check_sources(&ws, true, &CancelToken::new())
        .unwrap()
        .iter()
        .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)));

    e.s.remove_entries(&[id_of(&e.s, &ws, "f01.txt")]).unwrap();
    let out = e.s.finalize_removals(&ws);
    assert_eq!(out.files_removed, 1);
    assert!(out.bytes_freed > 0);
    assert!(!f.exists(), "now final: the stored copy is deleted");
    assert!(other.exists(), "other files are untouched");
    assert!(e.s.undo_remove(&ws).is_err());
}

#[test]
fn removing_a_folder_deletes_its_stored_files_and_prunes_empty_folders_but_keeps_the_workspace_folder() {
    let mut e = env();
    let ws = e.imported(1);
    let (one, two) = (e.path_of(&ws, "one.txt"), e.path_of(&ws, "two.txt"));
    let ws_dir = e.managed.join(&ws);
    e.s.remove_entries(&[id_of(&e.s, &ws, "Dir")]).unwrap();
    assert!(one.exists() && two.exists());
    assert_eq!(e.s.finalize_removals(&ws).files_removed, 2);
    assert!(!one.exists() && !two.exists());
    assert!(!ws_dir.join("Dir").exists(), "empty Dir/ and Dir/Deep/ are pruned");
    assert!(ws_dir.join("f00.txt").exists());
    assert!(ws_dir.exists());
}

#[test]
fn files_shared_with_a_duplicate_workspace_are_never_deleted() {
    let mut e = env();
    let a = e.imported(2);
    let b = e.s.duplicate_workspace(&a, "Copy").unwrap().id;
    let shared = e.path_of(&a, "f00.txt");
    e.s.remove_entries(&[id_of(&e.s, &a, "f00.txt")]).unwrap();
    assert_eq!(e.s.finalize_removals(&a).files_removed, 0);
    assert!(shared.exists(), "the duplicate still uses it");
    assert!(e
        .s
        .check_sources(&b, true, &CancelToken::new())
        .unwrap()
        .iter()
        .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)));
    e.s.remove_entries(&[id_of(&e.s, &b, "f00.txt")]).unwrap();
    assert_eq!(e.s.finalize_removals(&b).files_removed, 1);
    assert!(!shared.exists(), "last user gone");
}

#[test]
fn a_file_is_kept_when_another_item_spells_its_path_differently() {
    let mut e = env();
    let a = e.imported(1);
    let f = e.path_of(&a, "f00.txt");
    let other = e.s.create_workspace("Other", "", None).unwrap().id;
    // Same file, written the way another platform/normaliser might: different case and doubled slashes.
    let weird = f.to_string_lossy().to_uppercase().replace('/', "//");
    let scan = parcel_workspace::collect::ScanReport {
        items: vec![parcel_workspace::collect::ScannedItem {
            is_folder: false,
            dir: vec![],
            name: "x.txt".into(),
            source_path: Some(PathBuf::from(weird)),
            size: 1,
            sha256: Some("h".into()),
            mime: None,
            mtime_ms: None,
        }],
        warnings: vec![],
    };
    e.s.add_scanned(&other, None, &scan).unwrap();
    e.s.remove_entries(&[id_of(&e.s, &a, "f00.txt")]).unwrap();
    assert_eq!(e.s.finalize_removals(&a).files_removed, 0);
    assert!(f.exists(), "doubt keeps the file");
}

#[test]
fn history_is_limited_and_evicted_removals_become_final() {
    let mut e = env();
    let n = MAX_UNDO + 3;
    let ws = e.imported(n);
    let paths: Vec<PathBuf> = (0..n).map(|i| e.path_of(&ws, &format!("f{i:02}.txt"))).collect();
    for i in 0..n {
        e.s.remove_entries(&[id_of(&e.s, &ws, &format!("f{i:02}.txt"))]).unwrap();
    }
    assert_eq!(e.s.list_removals(&ws).len(), MAX_UNDO);
    for (i, p) in paths.iter().enumerate() {
        assert_eq!(p.exists(), i >= 3, "removal #{i}: the 3 oldest are final and deleted, the rest are still undoable");
    }
    // The newest is still fully undoable.
    e.s.undo_remove(&ws).unwrap();
    assert!(names(&e.s, &ws).contains(&format!("f{:02}.txt", n - 1)));
}

#[test]
fn nothing_outside_parcels_storage_is_ever_deleted() {
    let mut e = env();
    let ws = e.s.create_workspace("Mine", "", None).unwrap().id;
    let mine = w(e.t.path(), "docs/mine.txt", b"my own file");
    let loose = w(&e.managed, "loose.txt", b"in the root but not in a workspace folder");
    let wrongdir = w(&e.managed, "not-a-uuid/x.txt", b"x");
    add(&mut e.s, &ws, None, &[mine.clone(), loose.clone(), wrongdir.clone()]);
    // Even if the origin says "imported", only files below <root>/<uuid>/ may go.
    let db = rusqlite::Connection::open(e.t.path().join("db/parcel.db")).unwrap();
    db.execute("UPDATE entries SET origin='imported'", []).unwrap();
    drop(db);
    let ids: Vec<String> = e.s.list_entries(&ws).unwrap().into_iter().map(|x| x.id).collect();
    e.s.remove_entries(&ids).unwrap();
    assert_eq!(e.s.finalize_removals(&ws).files_removed, 0);
    assert!(mine.exists() && loose.exists() && wrongdir.exists());
}

#[test]
fn a_uuid_named_folder_outside_parcels_storage_is_never_touched() {
    // `foreign` has the same length as `managed`, so a length-based slice of the path would look like <root>/<uuid>/...
    let mut e = env();
    let ws = e.s.create_workspace("Mine", "", None).unwrap().id;
    let keep = w(e.t.path(), &format!("foreign/{}/keep.txt", uuid::Uuid::new_v4()), b"user data in a UUID-named folder");
    add(&mut e.s, &ws, None, std::slice::from_ref(&keep));
    let db = rusqlite::Connection::open(e.t.path().join("db/parcel.db")).unwrap();
    db.execute("UPDATE entries SET origin='imported'", []).unwrap();
    drop(db);
    e.s.remove_entries(&[id_of(&e.s, &ws, "keep.txt")]).unwrap();
    assert_eq!(e.s.finalize_removals(&ws).files_removed, 0);
    assert!(keep.exists());
}

#[test]
fn without_a_managed_root_no_file_is_ever_deleted() {
    let mut e = env();
    let ws = e.imported(1);
    let f = e.path_of(&ws, "f00.txt");
    e.s.set_managed_root(None);
    e.s.remove_entries(&[id_of(&e.s, &ws, "f00.txt")]).unwrap();
    assert_eq!(e.s.finalize_removals(&ws).files_removed, 0);
    assert!(f.exists());
}

#[cfg(unix)]
#[test]
fn symlinks_are_never_followed_or_deleted() {
    let mut e = env();
    let ws = e.imported(1);
    let precious = w(e.t.path(), "precious/keep.txt", b"keep");
    let link = e.managed.join(&ws).join("link.txt");
    std::os::unix::fs::symlink(&precious, &link).unwrap();
    let f = e.path_of(&ws, "f00.txt");
    // Point an imported entry at the symlink, remove it, finalize.
    let db = rusqlite::Connection::open(e.t.path().join("db/parcel.db")).unwrap();
    db.execute("UPDATE entries SET source_path=?1 WHERE name='f00.txt'", [link.to_string_lossy().to_string()]).unwrap();
    drop(db);
    e.s.remove_entries(&[id_of(&e.s, &ws, "f00.txt")]).unwrap();
    assert_eq!(e.s.finalize_removals(&ws).files_removed, 0);
    assert!(precious.exists() && link.exists() && f.exists());
}

#[test]
fn leaving_everything_pending_then_exiting_finalizes_all_workspaces() {
    let mut e = env();
    let a = e.imported(1);
    let f = e.path_of(&a, "f00.txt");
    e.s.remove_entries(&[id_of(&e.s, &a, "f00.txt")]).unwrap();
    assert!(f.exists());
    assert_eq!(e.s.finalize_all_removals().files_removed, 1);
    assert!(!f.exists());
}

#[test]
fn relinking_an_imported_item_to_your_own_file_frees_the_stored_copy_unless_shared() {
    let mut e = env();
    let a = e.imported(2);
    let b = e.s.duplicate_workspace(&a, "Copy").unwrap().id;
    let stored0 = e.path_of(&a, "f00.txt");
    let mine = w(e.t.path(), "mine/f00.txt", b"content 0"); // identical content
                                                            // Shared with the duplicate: relinking in A must not delete it.
    e.s.relink_source(&id_of(&e.s, &a, "f00.txt"), &mine, false).unwrap();
    assert!(stored0.exists());
    let moved = e.s.list_entries(&a).unwrap().into_iter().find(|x| x.name == "f00.txt").unwrap();
    assert_eq!((moved.origin.as_str(), moved.source_path.as_deref()), ("source", Some(mine.to_str().unwrap())));
    // Now relink the duplicate's too: nothing uses the stored copy any more.
    e.s.relink_source(&id_of(&e.s, &b, "f00.txt"), &mine, false).unwrap();
    assert!(!stored0.exists());
    assert!(e.path_of(&a, "f01.txt").exists());
}

// ---------------- Settings → Storage ----------------
#[test]
fn storage_scan_reports_and_removal_deletes_only_unused_files() {
    let mut e = env();
    let ws = e.imported(2);
    let used = e.path_of(&ws, "f00.txt");
    let stray = w(&e.managed.join(&ws), "Leftover/stray.bin", &vec![7u8; 1000]);
    let orphan_id = uuid::Uuid::new_v4().to_string();
    let orphan = w(&e.managed, &format!("{orphan_id}/old.txt"), b"old");
    let foreign = w(&e.managed, "not-ours/keep.txt", b"keep");
    let protected_id = uuid::Uuid::new_v4().to_string();
    let protected = w(&e.managed, &format!("{protected_id}/staging.txt"), b"in progress");

    let rep = e.s.scan_managed_storage().unwrap();
    assert_eq!(rep.unused_files, 1, "{rep:?}");
    assert_eq!(rep.unused_bytes, 1000);
    assert!(rep.unused_examples[0].ends_with("stray.bin"));
    assert_eq!(rep.orphan_folders, 2);
    assert!(rep.files >= 5 && rep.bytes > 1000);

    let prot: HashSet<String> = [protected_id].into_iter().collect();
    let out = e.s.remove_unused_storage(&prot).unwrap();
    assert!(out.files_removed >= 2);
    assert!(!stray.exists() && !orphan.exists());
    assert!(used.exists() && foreign.exists() && protected.exists());
    let after = e.s.scan_managed_storage().unwrap();
    assert_eq!(after.unused_files, 0);
    assert_eq!(after.orphan_folders, 1, "only the protected in-progress folder remains");
}

#[test]
fn pending_undo_files_count_as_used_in_the_storage_scan_and_cleanup() {
    let mut e = env();
    let ws = e.imported(2);
    let f = e.path_of(&ws, "f00.txt");
    e.s.remove_entries(&[id_of(&e.s, &ws, "f00.txt")]).unwrap();
    assert_eq!(e.s.scan_managed_storage().unwrap().unused_files, 0);
    assert_eq!(e.s.remove_unused_storage(&HashSet::new()).unwrap().files_removed, 0);
    assert!(f.exists());
    e.s.undo_remove(&ws).unwrap();
    assert!(e
        .s
        .check_sources(&ws, true, &CancelToken::new())
        .unwrap()
        .iter()
        .all(|c| matches!(c.status, parcel_core::intel::SourceStatus::Unchanged)));
}

#[cfg(unix)]
#[test]
fn storage_scan_ignores_symlinks() {
    let mut e = env();
    let ws = e.imported(1);
    let outside = w(e.t.path(), "outside/big.bin", &vec![1u8; 5000]);
    std::os::unix::fs::symlink(&outside, e.managed.join(&ws).join("l.bin")).unwrap();
    std::os::unix::fs::symlink(e.t.path().join("outside"), e.managed.join(&ws).join("ldir")).unwrap();
    let rep = e.s.scan_managed_storage().unwrap();
    assert_eq!(rep.unused_files, 0);
    e.s.remove_unused_storage(&HashSet::new()).unwrap();
    assert!(outside.exists());
}
