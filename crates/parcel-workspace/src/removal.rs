//! Undoable removal of entries, and cleanup of the stored copies of imported files.
//!
//! * Removing entries deletes their database rows in one transaction and keeps a snapshot **in memory**
//!   (no schema change, nothing persisted). `undo_remove` re-inserts that snapshot atomically.
//! * A file that Parcel itself stores (`origin = "imported"`) is deleted only when the removal becomes final:
//!   the history entry is evicted, the workspace is left, or the app exits. A file is only ever deleted when it
//!   lies under `<managed root>/<workspace uuid>/` and no entry in ANY workspace (nor a pending undo) references it.
//! * Every path comparison goes through [`norm_path`], which erases Windows-vs-Unix spelling differences
//!   (`\\?\` prefix, slashes, case). Differences can only make two paths compare *equal*, which keeps files.
use crate::error::{Result, WsError};
use crate::import::is_workspace_dir_name;
use crate::models::{DepRow, Entry, RefRow};
use crate::store::{insert_entry_row, now, Store};
use rusqlite::params;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

/// How many removals per workspace can be undone. Older ones become final (their stored files are deleted).
pub const MAX_UNDO: usize = 20;

pub(crate) struct UndoEntry {
    pub id: u64,
    pub label: String,
    entries: Vec<Entry>,
    refs: Vec<RefRow>,
    deps: Vec<DepRow>,
    /// (entry id, attached_to) for notes outside the removed set that were attached to removed entries.
    attachments: Vec<(String, String)>,
    /// Stored copies of imported files owned by the removed entries.
    files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RemovalReceipt {
    pub id: u64,
    pub workspace_id: String,
    /// Items removed, including everything inside removed folders.
    pub removed: usize,
    /// Stored copies of imported files that will be deleted when the removal becomes final.
    pub stored_files: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RemovalSummary {
    pub id: u64,
    pub label: String,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct UndoOutcome {
    pub restored: usize,
    /// Top-level items renamed because the name was taken in the meantime.
    pub renamed: usize,
    /// Top-level items put at the workspace root because their folder no longer exists.
    pub relocated: usize,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct StorageOutcome {
    pub files_removed: usize,
    pub bytes_freed: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StorageReport {
    pub files: usize,
    pub bytes: u64,
    pub unused_files: usize,
    pub unused_bytes: u64,
    /// A few unused files (relative to Parcel's storage folder), so the user can see what would be removed.
    pub unused_examples: Vec<String>,
    /// UUID-named folders with no workspace and no references.
    pub orphan_folders: usize,
}

/// Canonical form used ONLY to compare paths for equality. Not a filesystem path.
pub fn norm_path(p: &str) -> String {
    let p = match p.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest,
        _ => p,
    };
    let unified = p.replace('\\', "/").to_lowercase();
    let mut out = String::with_capacity(unified.len());
    if unified.starts_with('/') {
        out.push('/');
    }
    out.push_str(&unified.split('/').filter(|c| !c.is_empty() && *c != ".").collect::<Vec<_>>().join("/"));
    out
}

/// True when `child` lies strictly below `root` (both already normalised).
fn is_below(child: &str, root: &str) -> bool {
    let root = root.trim_end_matches('/');
    child.len() > root.len() + 1 && child.starts_with(root) && child.as_bytes()[root.len()] == b'/'
}

impl Store {
    /// Tell the store where Parcel keeps imported files. Without this, no file is ever deleted.
    pub fn set_managed_root(&self, root: Option<PathBuf>) {
        *self.managed_root.lock().unwrap() = root;
    }

    fn managed_root(&self) -> Option<PathBuf> {
        self.managed_root.lock().unwrap().clone()
    }

    /// Remove entries (and everything inside removed folders) from a workspace. Original files are never touched,
    /// and the removal can be undone until it becomes final. Atomic.
    pub fn remove_entries(&self, ids: &[String]) -> Result<RemovalReceipt> {
        if ids.is_empty() {
            return Err(WsError::Invalid("nothing to remove".into()));
        }
        let ws = self.get_entry(&ids[0])?.workspace_id;
        let all = self.list_entries(&ws)?;
        let by_id: HashMap<&str, &Entry> = all.iter().map(|e| (e.id.as_str(), e)).collect();
        for id in ids {
            match by_id.get(id.as_str()) {
                Some(_) => {}
                None if self.get_entry(id).is_ok() => return Err(WsError::Invalid("items belong to different workspaces".into())),
                None => return Err(WsError::NotFound(format!("entry {id}"))),
            }
        }
        // The removal set: the selection plus all descendants.
        let mut children: HashMap<&str, Vec<&Entry>> = HashMap::new();
        for e in &all {
            if let Some(p) = &e.parent_id {
                children.entry(p.as_str()).or_default().push(e);
            }
        }
        let mut set: Vec<&Entry> = vec![];
        let mut seen: HashSet<&str> = HashSet::new();
        let mut stack: Vec<&Entry> = ids.iter().map(|i| by_id[i.as_str()]).collect();
        while let Some(e) = stack.pop() {
            if seen.insert(e.id.as_str()) {
                set.push(e);
                stack.extend(children.get(e.id.as_str()).into_iter().flatten().copied());
            }
        }
        let in_set: HashSet<&str> = seen;
        let refs: Vec<RefRow> =
            self.list_references(&ws)?.into_iter().filter(|r| r.entry_id.as_deref().is_some_and(|i| in_set.contains(i))).collect();
        let deps: Vec<DepRow> = self.list_dependencies(&ws)?.into_iter().filter(|d| in_set.contains(d.from_entry.as_str())).collect();
        let attachments: Vec<(String, String)> = all
            .iter()
            .filter(|e| !in_set.contains(e.id.as_str()))
            .filter_map(|e| e.attached_to.as_ref().filter(|a| in_set.contains(a.as_str())).map(|a| (e.id.clone(), a.clone())))
            .collect();
        let files: Vec<String> =
            set.iter().filter(|e| e.kind == "file" && e.origin == "imported").filter_map(|e| e.source_path.clone()).collect();
        let first_name = by_id[ids[0].as_str()].name.clone();
        let label = if ids.len() == 1 { first_name } else { format!("{} items", ids.len()) };
        let entry = UndoEntry {
            id: self.undo_seq.fetch_add(1, Ordering::SeqCst) + 1,
            label,
            entries: set.iter().map(|e| (*e).clone()).collect(),
            refs,
            deps,
            attachments,
            files,
        };
        let receipt =
            RemovalReceipt { id: entry.id, workspace_id: ws.clone(), removed: entry.entries.len(), stored_files: entry.files.len() };

        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute("DELETE FROM entries WHERE id=?1 AND workspace_id=?2", params![id, ws])?;
            // children cascade
        }
        tx.commit()?;
        self.conn.execute("UPDATE workspaces SET modified_at=?2 WHERE id=?1", params![ws, now()])?;
        self.log(
            Some(&ws),
            "entry.removed",
            &format!("Removed {} item(s) from the workspace (originals untouched)", receipt.removed),
            serde_json::json!({"undoable": true}),
        );
        let evicted = {
            let mut g = self.undo.lock().unwrap();
            let stack = g.entry(ws).or_default();
            stack.push(entry);
            let excess = stack.len().saturating_sub(MAX_UNDO);
            stack.drain(..excess).collect::<Vec<_>>()
        };
        for old in evicted {
            self.finalize_files(&old.files);
        }
        Ok(receipt)
    }

    /// Removals that can still be undone in this workspace, newest first.
    pub fn list_removals(&self, ws: &str) -> Vec<RemovalSummary> {
        let g = self.undo.lock().unwrap();
        g.get(ws)
            .map(|v| v.iter().rev().map(|u| RemovalSummary { id: u.id, label: u.label.clone(), removed: u.entries.len() }).collect())
            .unwrap_or_default()
    }

    /// Undo the most recent removal in `ws`. Atomic: either everything comes back or nothing does.
    pub fn undo_remove(&self, ws: &str) -> Result<UndoOutcome> {
        let workspace = self.get_workspace(ws)?;
        if workspace.deleted_at.is_some() {
            return Err(WsError::Invalid("restore the workspace from the trash before undoing".into()));
        }
        let (id, snapshot) = {
            let g = self.undo.lock().unwrap();
            let u = g.get(ws).and_then(|v| v.last()).ok_or_else(|| WsError::Invalid("there is nothing to undo".into()))?;
            (u.id, (u.entries.clone(), u.refs.clone(), u.deps.clone(), u.attachments.clone()))
        };
        let (entries, refs, deps, attachments) = snapshot;
        let ids: HashSet<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        let by_id: HashMap<&str, &Entry> = entries.iter().map(|e| (e.id.as_str(), e)).collect();
        let depth = |e: &Entry| {
            let (mut d, mut cur) = (0usize, e);
            while let Some(p) = cur.parent_id.as_deref().and_then(|p| by_id.get(p)) {
                d += 1;
                cur = p;
            }
            d
        };
        let mut ordered: Vec<&Entry> = entries.iter().collect();
        ordered.sort_by_key(|e| depth(e));

        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
        let mut out = UndoOutcome::default();
        let exists = |sql: &str, key: &str| -> Result<bool> { Ok(tx.query_row(sql, [key], |r| r.get::<_, bool>(0))?) };
        for e in ordered {
            if exists("SELECT EXISTS(SELECT 1 FROM entries WHERE id=?1)", &e.id)? {
                return Err(WsError::Invalid("these items were already restored".into()));
            }
            let mut row = e.clone();
            let is_top = e.parent_id.as_deref().is_none_or(|p| !ids.contains(p));
            if is_top {
                if let Some(p) = &e.parent_id {
                    let parent_ok: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM entries WHERE id=?1 AND workspace_id=?2 AND kind='folder')",
                        params![p, ws],
                        |r| r.get(0),
                    )?;
                    if !parent_ok {
                        row.parent_id = None;
                        out.relocated += 1;
                    }
                }
                let unique = self.unique_name(ws, row.parent_id.as_deref(), &row.name, None)?;
                if unique != row.name {
                    row.name = unique;
                    out.renamed += 1;
                }
            }
            // An attachment target that no longer exists anywhere would break the foreign key.
            if let Some(a) = &row.attached_to {
                let ok = ids.contains(a.as_str()) || exists("SELECT EXISTS(SELECT 1 FROM entries WHERE id=?1)", a)?;
                if !ok {
                    row.attached_to = None;
                }
            }
            insert_entry_row(&tx, &row)?;
            out.restored += 1;
        }
        for r in &refs {
            if exists("SELECT EXISTS(SELECT 1 FROM refs WHERE id=?1)", &r.id)? {
                // The reference outlived the removal but lost its link to the item; reconnect it.
                tx.execute("UPDATE refs SET entry_id=?2 WHERE id=?1 AND entry_id IS NULL", params![r.id, r.entry_id])?;
            } else {
                tx.execute(
                    "INSERT INTO refs(id,workspace_id,title,url,description,category,entry_id,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![r.id, r.workspace_id, r.title, r.url, r.description, r.category, r.entry_id, r.sort_order],
                )?;
            }
        }
        for d in &deps {
            if !exists("SELECT EXISTS(SELECT 1 FROM dependencies WHERE id=?1)", &d.id)? {
                tx.execute(
                    "INSERT INTO dependencies(id,workspace_id,from_entry,kind,target,description,required) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![d.id, d.workspace_id, d.from_entry, d.kind, d.target, d.description, d.required],
                )?;
            }
        }
        for (note, target) in &attachments {
            tx.execute("UPDATE entries SET attached_to=?2 WHERE id=?1 AND attached_to IS NULL", params![note, target])?;
        }
        tx.commit()?;
        {
            let mut g = self.undo.lock().unwrap();
            if let Some(v) = g.get_mut(ws) {
                v.retain(|u| u.id != id);
            }
        }
        self.conn.execute("UPDATE workspaces SET modified_at=?2 WHERE id=?1", params![ws, now()])?;
        self.log(Some(ws), "entry.restored", &format!("Restored {} item(s) removed earlier", out.restored), serde_json::Value::Null);
        Ok(out)
    }

    /// Make every pending removal in `ws` final and delete the stored copies that nothing uses any more.
    pub fn finalize_removals(&self, ws: &str) -> StorageOutcome {
        let drained = self.undo.lock().unwrap().remove(ws).unwrap_or_default();
        let files: Vec<String> = drained.into_iter().flat_map(|u| u.files).collect();
        self.finalize_files(&files)
    }

    /// Same for all workspaces (used when the app exits).
    pub fn finalize_all_removals(&self) -> StorageOutcome {
        let drained: Vec<UndoEntry> = self.undo.lock().unwrap().drain().flat_map(|(_, v)| v).collect();
        let files: Vec<String> = drained.into_iter().flat_map(|u| u.files).collect();
        self.finalize_files(&files)
    }

    fn referenced_paths(&self) -> Result<HashSet<String>> {
        let mut st = self.conn.prepare("SELECT source_path FROM entries WHERE source_path IS NOT NULL")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).map(|p| norm_path(&p)).collect())
    }

    fn pinned_paths(&self) -> HashSet<String> {
        self.undo.lock().unwrap().values().flatten().flat_map(|u| u.files.iter()).map(|p| norm_path(p)).collect()
    }

    /// Delete stored copies that are unreferenced and unpinned. Best-effort and conservative: any doubt keeps the file.
    pub(crate) fn finalize_files(&self, files: &[String]) -> StorageOutcome {
        let mut out = StorageOutcome::default();
        let (Some(root), false) = (self.managed_root(), files.is_empty()) else { return out };
        let Ok(referenced) = self.referenced_paths() else { return out };
        let pinned = self.pinned_paths();
        let root_n = norm_path(&root.to_string_lossy());
        for f in files {
            let n = norm_path(f);
            if referenced.contains(&n) || pinned.contains(&n) || !is_below(&n, &root_n) {
                continue;
            }
            // The first component below the root must be a workspace folder (a lower-case UUID).
            let rel = &n[root_n.trim_end_matches('/').len() + 1..];
            let first = rel.split('/').next().unwrap_or("");
            if !is_workspace_dir_name(first) || !rel.contains('/') {
                continue;
            }
            let p = Path::new(f);
            match fs::symlink_metadata(p) {
                Ok(m)
                    if m.is_file()
                        && !m.file_type().is_symlink()
                        && fs::remove_file(p).is_ok() =>
                {
                   out.files_removed += 1;
                    out.bytes_freed += m.len();
                   prune_empty_parents(p, &root, first);
                }
                 _ => {}
            }
        }
        if out.files_removed > 0 {
            self.log(
                None,
                "storage.cleaned",
                &format!("Removed {} stored file(s) no longer used", out.files_removed),
                serde_json::json!({"bytes": out.bytes_freed}),
            );
        }
        out
    }

    /// Dry-run: how much of Parcel's imported-file storage is in use, and what nothing references.
    pub fn scan_managed_storage(&self) -> Result<StorageReport> {
        let mut rep = StorageReport::default();
        let Some(root) = self.managed_root() else { return Ok(rep) };
        let referenced = self.referenced_paths()?;
        let pinned = self.pinned_paths();
        let Ok(rd) = fs::read_dir(&root) else { return Ok(rep) };
        for dir in rd.flatten() {
            let name = dir.file_name().to_string_lossy().into_owned();
            let is_real_dir = fs::symlink_metadata(dir.path()).map(|m| m.is_dir() && !m.file_type().is_symlink()).unwrap_or(false);
            if !is_real_dir || !is_workspace_dir_name(&name) {
                continue;
            }
            let has_ws: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)", [&name], |r| r.get(0))?;
            let any_ref: bool =
                self.conn
                    .query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE instr(lower(source_path), ?1) > 0)", [&name], |r| r.get(0))?;
            if !has_ws && !any_ref {
                rep.orphan_folders += 1;
                continue;
            }
            let mut files = vec![];
            collect_files(&dir.path(), &mut files);
            for (path, size) in files {
                rep.files += 1;
                rep.bytes += size;
                let n = norm_path(&path.to_string_lossy());
                if !referenced.contains(&n) && !pinned.contains(&n) {
                    rep.unused_files += 1;
                    rep.unused_bytes += size;
                    if rep.unused_examples.len() < 12 {
                        rep.unused_examples.push(path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().into_owned());
                    }
                }
            }
        }
        Ok(rep)
    }

    /// Delete what [`scan_managed_storage`] reports as unused (recomputed now; a stale report is never trusted),
    /// then remove folders for workspaces that no longer exist. `protected` = imports in progress.
    pub fn remove_unused_storage(&self, protected: &HashSet<String>) -> Result<StorageOutcome> {
        let mut out = StorageOutcome::default();
        let Some(root) = self.managed_root() else { return Ok(out) };
        let referenced = self.referenced_paths()?;
        let pinned = self.pinned_paths();
        let mut doomed: Vec<String> = vec![];
        if let Ok(rd) = fs::read_dir(&root) {
            for dir in rd.flatten() {
                let name = dir.file_name().to_string_lossy().into_owned();
                let is_real_dir = fs::symlink_metadata(dir.path()).map(|m| m.is_dir() && !m.file_type().is_symlink()).unwrap_or(false);
                if !is_real_dir || !is_workspace_dir_name(&name) || protected.contains(&name) {
                    continue;
                }
                let has_ws: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)", [&name], |r| r.get(0))?;
                if !has_ws {
                    continue; // whole-folder cleanup below decides about those
                }
                let mut files = vec![];
                collect_files(&dir.path(), &mut files);
                for (path, _) in files {
                    let n = norm_path(&path.to_string_lossy());
                    if !referenced.contains(&n) && !pinned.contains(&n) {
                        doomed.push(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
        let r = self.finalize_files(&doomed);
        out.files_removed += r.files_removed;
        out.bytes_freed += r.bytes_freed;
        let dirs = self.cleanup_managed_dirs(&root, protected)?;
        out.bytes_freed += dirs.bytes_freed;
        out.files_removed += dirs.removed.len();
        Ok(out)
    }
}

/// Regular files below `dir` (symlinks are neither followed nor listed).
fn collect_files(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(m) = fs::symlink_metadata(e.path()) else { continue };
        if m.file_type().is_symlink() {
            continue;
        }
        if m.is_dir() {
            collect_files(&e.path(), out);
        } else if m.is_file() {
            out.push((e.path(), m.len()));
        }
    }
}

/// Remove now-empty folders upward from a deleted file, never removing the workspace folder itself.
fn prune_empty_parents(file: &Path, root: &Path, ws_dir_name: &str) {
    let ws_dir = norm_path(&root.join(ws_dir_name).to_string_lossy());
    let mut cur = file.parent();
    while let Some(d) = cur {
        if !is_below(&norm_path(&d.to_string_lossy()), &ws_dir) || fs::remove_dir(d).is_err() {
            break;
        }
        cur = d.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_unix_spellings_of_one_path_compare_equal() {
        let a = norm_path(r"\\?\C:\Users\Me\AppData\imported\ABC\Sub\A.TXT");
        let b = norm_path("c:/users/me/appdata/imported/abc//sub/./a.txt");
        assert_eq!(a, b);
        assert_ne!(norm_path(r"\\?\UNC\srv\share\f"), norm_path(r"\\srv\share\f"));
        assert_eq!(norm_path("/home/x/y"), "/home/x/y");
    }

    #[test]
    fn is_below_requires_a_path_boundary() {
        assert!(is_below("/a/b/c", "/a/b"));
        assert!(!is_below("/a/bc/d", "/a/b"));
        assert!(!is_below("/a/b", "/a/b"));
        assert!(is_below("c:/x/y", "c:/x/"));
    }
}
