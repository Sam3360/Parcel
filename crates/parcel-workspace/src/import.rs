//! Two-phase package import.
//!
//! Phase 1 ([`StagedImport::prepare`]) opens the package, extracts and verifies its files into an
//! app-managed folder and reads notes. It never touches the database, so callers need not hold any lock.
//! Phase 2 ([`Store::commit_import`]) creates the workspace and every entry in ONE transaction: either the
//! whole workspace exists, or nothing does. A failed or cancelled import removes its extracted files.
use crate::error::{Result, WsError};
use crate::models::Workspace;
use crate::store::{new_id, now, Store};
use parcel_core::cancel::{CancelToken, ProgressFn};
use parcel_core::intel::mtime_ms;
use parcel_core::limits::ResourceLimits;
use parcel_core::manifest::{DependencyKind, EntryKind, Manifest};
use parcel_core::pathsafe::safe_join;
use parcel_core::reader::{ConflictPolicy, ExtractOptions, PackageReader};
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct StagedImport {
    pub workspace_id: String,
    pub root: PathBuf,
    manifest: Manifest,
    notes: String,
    note_bodies: HashMap<String, String>,
    files: HashMap<String, (PathBuf, Option<i64>)>,
    pkg_path: PathBuf,
    pkg_size: u64,
    created_root: bool,
}

impl StagedImport {
    pub fn package_name(&self) -> &str {
        &self.manifest.name
    }

    /// Extract and verify the package's files into `managed_dir/<workspace_id>/`. On any failure
    /// (including cancellation) everything extracted so far is removed before the error is returned.
    pub fn prepare(
        pkg: &Path,
        managed_dir: &Path,
        workspace_id: &str,
        limits: &ResourceLimits,
        cancel: &CancelToken,
        progress: ProgressFn<'_>,
    ) -> Result<Self> {
        if !is_workspace_dir_name(workspace_id) {
            return Err(WsError::Invalid("invalid workspace id".into()));
        }
        let root = managed_dir.join(workspace_id);
        if root.exists() {
            return Err(WsError::Invalid("import folder already exists".into()));
        }
        let staged = Self::stage(pkg, &root, workspace_id, limits, cancel, progress);
        if staged.is_err() {
            let _ = fs::remove_dir_all(&root);
        }
        staged
    }

    fn stage(
        pkg: &Path,
        root: &Path,
        workspace_id: &str,
        limits: &ResourceLimits,
        cancel: &CancelToken,
        progress: ProgressFn<'_>,
    ) -> Result<Self> {
        let mut reader = PackageReader::open(pkg, limits)?;
        let manifest = reader.manifest().clone();
        let notes = reader.notes()?;
        let mut note_bodies = HashMap::new();
        for e in manifest.entries.iter().filter(|e| e.kind == EntryKind::Note) {
            cancel.check()?;
            let mut buf = Vec::new();
            reader.read_entry_verified(&e.id, &mut buf, cancel)?;
            note_bodies.insert(e.id.clone(), String::from_utf8_lossy(&buf).into_owned());
        }
        // Only real files are extracted; notes live in the database and links are just addresses.
        let file_ids: Vec<String> = manifest.entries.iter().filter(|e| e.kind == EntryKind::File).map(|e| e.id.clone()).collect();
        let mut files = HashMap::new();
        let mut created_root = false;
        if !file_ids.is_empty() {
            let opts = ExtractOptions { selection: Some(file_ids.clone()), conflict: ConflictPolicy::Fail, limits: limits.clone() };
            reader.extract(root, &opts, cancel, progress)?;
            created_root = true;
            let canon = root.canonicalize()?;
            for e in manifest.entries.iter().filter(|e| e.kind == EntryKind::File) {
                let src = safe_join(&canon, &e.path, limits)?;
                let m = mtime_ms(&src);
                files.insert(e.id.clone(), (src, m));
            }
        }
        Ok(Self {
            workspace_id: workspace_id.to_string(),
            root: root.to_path_buf(),
            manifest,
            notes,
            note_bodies,
            files,
            pkg_path: pkg.to_path_buf(),
            pkg_size: reader.file_size(),
            created_root,
        })
    }

    /// Remove extracted files. Safe to call more than once.
    pub fn discard(&self) {
        if self.created_root || self.root.exists() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

/// Managed folders are named after the workspace's UUID; nothing else is ever treated as ours.
pub fn is_workspace_dir_name(name: &str) -> bool {
    uuid::Uuid::parse_str(name).is_ok() && name.len() == 36 && name == name.to_lowercase() && !name.contains(['/', '\\'])
}

impl Store {
    /// Create the workspace for a staged import in a single transaction (all or nothing).
    pub fn commit_import(&mut self, staged: &StagedImport) -> Result<Workspace> {
        let m = &staged.manifest;
        let ws_id = &staged.workspace_id;
        let ts = now();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO workspaces(id,name,description,notes,metadata,package_id,revision,created_at,modified_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",
            params![ws_id, m.name.trim(), m.description, staged.notes, serde_json::to_string(&m.metadata)?, m.package_id, m.revision as i64, ts],
        )?;
        let mut id_map: HashMap<String, String> = HashMap::new();
        {
            let mut ins = tx.prepare(
                "INSERT INTO entries(id,workspace_id,parent_id,kind,name,description,url,content,sort_order,origin,source_path,size,sha256,mime,source_mtime_ms,tags,metadata,created_at,modified_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?18)",
            )?;
            let mut folders: HashMap<String, String> = HashMap::new(); // lower-case path -> db id
            let mut next_order: HashMap<Option<String>, i64> = HashMap::new();
            let mut order_for = |parent: &Option<String>| -> i64 {
                let o = next_order.entry(parent.clone()).or_insert(0);
                *o += 1;
                *o
            };
            let mut entries: Vec<&parcel_core::manifest::Entry> = m.entries.iter().collect();
            entries.sort_by_key(|e| e.path.matches('/').count());
            for e in entries {
                let (parent_path, name) = match e.path.rsplit_once('/') {
                    Some((p, n)) => (Some(p), n),
                    None => (None, e.path.as_str()),
                };
                // Make sure every ancestor folder exists (packages may omit explicit folder entries).
                let mut parent_id: Option<String> = None;
                if let Some(pp) = parent_path {
                    let mut acc = String::new();
                    for comp in pp.split('/') {
                        if !acc.is_empty() {
                            acc.push('/');
                        }
                        acc.push_str(comp);
                        let key = acc.to_lowercase();
                        if let Some(id) = folders.get(&key) {
                            parent_id = Some(id.clone());
                        } else {
                            let id = new_id();
                            let ord = order_for(&parent_id);
                            ins.execute(params![
                                id,
                                ws_id,
                                parent_id,
                                "folder",
                                comp,
                                "",
                                None::<String>,
                                None::<String>,
                                ord,
                                "source",
                                None::<String>,
                                0i64,
                                None::<String>,
                                None::<String>,
                                None::<i64>,
                                "[]",
                                "{}",
                                ts
                            ])?;
                            folders.insert(key, id.clone());
                            parent_id = Some(id);
                        }
                    }
                }
                let id = if e.kind == EntryKind::Folder {
                    let key = e.path.to_lowercase();
                    if let Some(existing) = folders.get(&key) {
                        // Defensive: a folder already created as an ancestor of an earlier entry.
                        id_map.insert(e.id.clone(), existing.clone());
                        continue;
                    }
                    let id = new_id();
                    folders.insert(key, id.clone());
                    id
                } else {
                    new_id()
                };
                let attrs_tags = serde_json::to_string(&e.tags)?;
                let attrs_meta =
                    serde_json::to_string(&e.metadata.iter().filter(|(k, _)| *k != "parcel.attached_to").collect::<BTreeMap<_, _>>())?;
                let (kind, content, url, origin, source, mtime, size, sha, mime) = match e.kind {
                    EntryKind::Folder => ("folder", None, None, "source", None, None, 0i64, None, None),
                    EntryKind::Note => {
                        let body = staged.note_bodies.get(&e.id).cloned().unwrap_or_default();
                        let sz = body.len() as i64;
                        ("note", Some(body), None, "source", None, None, sz, e.sha256.clone(), e.mime.clone())
                    }
                    EntryKind::Link => ("link", None, e.url.clone(), "source", None, None, 0, None, None),
                    EntryKind::File => {
                        let (p, mt) = staged.files.get(&e.id).ok_or_else(|| WsError::Invalid(format!("{} was not extracted", e.path)))?;
                        (
                            "file",
                            None,
                            None,
                            "imported",
                            Some(p.to_string_lossy().into_owned()),
                            *mt,
                            e.size as i64,
                            e.sha256.clone(),
                            e.mime.clone(),
                        )
                    }
                };
                let ord = order_for(&parent_id);
                ins.execute(params![
                    id,
                    ws_id,
                    parent_id,
                    kind,
                    name,
                    e.description,
                    url,
                    content,
                    ord,
                    origin,
                    source,
                    size,
                    sha,
                    mime,
                    mtime,
                    attrs_tags,
                    attrs_meta,
                    ts
                ])?;
                id_map.insert(e.id.clone(), id);
            }
        }
        // Second pass: attachments, then references and dependencies, which need the id mapping.
        for e in &m.entries {
            if let (Some(att), Some(new)) = (e.metadata.get("parcel.attached_to").and_then(|a| id_map.get(a)), id_map.get(&e.id)) {
                tx.execute("UPDATE entries SET attached_to=?2 WHERE id=?1", params![new, att])?;
            }
        }
        for (order, r) in (1i64..).zip(&m.references) {
            tx.execute(
                "INSERT INTO refs(id,workspace_id,title,url,description,category,entry_id,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![new_id(), ws_id, r.title, r.url, r.description, r.category, r.entry_id.as_ref().and_then(|i| id_map.get(i)), order],
            )?;
        }
        for d in &m.dependencies {
            let target = if d.kind == DependencyKind::Entry { id_map.get(&d.target).cloned() } else { Some(d.target.clone()) };
            if let (Some(from), Some(target)) = (id_map.get(&d.from_entry), target) {
                tx.execute(
                    "INSERT INTO dependencies(id,workspace_id,from_entry,kind,target,description,required) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        new_id(),
                        ws_id,
                        from,
                        if d.kind == DependencyKind::Entry { "entry" } else { "external" },
                        target,
                        d.description,
                        d.required
                    ],
                )?;
            }
        }
        tx.commit()?;
        self.add_recent_package(&staged.pkg_path.to_string_lossy(), &m.name, staged.pkg_size as i64)?;
        self.log(
            Some(ws_id),
            "package.imported",
            &format!("Imported package “{}”", m.name),
            serde_json::json!({"path": staged.pkg_path.to_string_lossy()}),
        );
        self.get_workspace(ws_id)
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CleanupReport {
    pub removed: Vec<PathBuf>,
    pub bytes_freed: u64,
    pub kept: usize,
    pub errors: Vec<String>,
}

impl Store {
    /// Remove app-managed import folders that nothing references any more.
    ///
    /// A folder is only ever removed when ALL of these hold: it is a direct child of `managed_root`, its
    /// name is a lower-case UUID, it is a real directory (not a symlink), it is not in `protected` (imports in
    /// progress), no workspace row (including trashed ones) has that id, and no entry anywhere (including in
    /// duplicated or trashed workspaces) has a source path containing that UUID. Anything uncertain is kept.
    pub fn cleanup_managed_dirs(&self, managed_root: &Path, protected: &HashSet<String>) -> Result<CleanupReport> {
        let mut rep = CleanupReport::default();
        let rd = match fs::read_dir(managed_root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(rep),
            Err(e) => return Err(e.into()),
        };
        for ent in rd.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            let path = ent.path();
            let is_real_dir = fs::symlink_metadata(&path).map(|m| m.is_dir() && !m.file_type().is_symlink()).unwrap_or(false);
            if !is_real_dir || !is_workspace_dir_name(&name) || protected.contains(&name) {
                rep.kept += 1;
                continue;
            }
            let has_ws: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?1)", [&name], |r| r.get(0))?;
            // Substring match on the globally unique UUID is robust to path-prefix differences
            // (e.g. Windows verbatim `\\?\` paths, case, separators); false positives only keep data.
            let referenced: bool =
                self.conn
                    .query_row("SELECT EXISTS(SELECT 1 FROM entries WHERE instr(lower(source_path), ?1) > 0)", [&name], |r| r.get(0))?;
            if has_ws || referenced {
                rep.kept += 1;
                continue;
            }
            let size = dir_size(&path);
            match fs::remove_dir_all(&path) {
                Ok(()) => {
                    rep.bytes_freed += size;
                    rep.removed.push(path);
                }
                Err(e) => rep.errors.push(format!("{}: {e}", path.display())),
            }
        }
        if !rep.removed.is_empty() {
            self.log(
                None,
                "storage.cleaned",
                &format!("Removed {} unused imported folder(s)", rep.removed.len()),
                serde_json::json!({"bytes": rep.bytes_freed}),
            );
        }
        Ok(rep)
    }
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = fs::read_dir(p) {
        for e in rd.flatten() {
            match e.metadata() {
                Ok(m) if m.is_dir() => total += dir_size(&e.path()),
                Ok(m) => total += m.len(),
                Err(_) => {}
            }
        }
    }
    total
}
