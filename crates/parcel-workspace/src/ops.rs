//! Higher-level workspace operations: adding scanned content, source checks, export, import.
use crate::collect::{ScanReport, ScannedItem};
use crate::error::{Result, SourceProblem, WsError};
use crate::import::StagedImport;
use crate::models::*;
use crate::store::{new_id, now, Store};
use crate::templates::{glob_match, ProfileSpec};
use parcel_core::cancel::{CancelToken, ProgressFn};
use parcel_core::intel::{self, DuplicateGroup, HashedItem, SourceRecord, SourceStatus};
use parcel_core::limits::ResourceLimits;
use parcel_core::manifest::{self as mf, Dependency, DependencyKind, EntryKind, Manifest, Reference};
use parcel_core::writer::{BuildInput, BuildItem, BuildReport, ExportOptions, ItemContent};
use rusqlite::params;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Default)]
pub struct AddReport {
    pub added: Vec<String>,
    pub already_collected: Vec<String>,
    pub renamed: Vec<(String, String)>,
    /// (new entry name, existing entry path) pairs whose content is byte-identical.
    pub content_duplicates: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceCheck {
    pub entry_id: String,
    pub path: String,
    pub source_path: String,
    #[serde(flatten)]
    pub status: SourceStatus,
}

#[derive(Debug, Clone, Default)]
pub struct ExportRequest {
    pub dest: PathBuf,
    pub overwrite: bool,
    /// Proceed even though some sources changed; the new content is recorded as the new baseline.
    pub accept_changed: bool,
}

pub struct PreparedExport {
    pub input: BuildInput,
    pub options: ExportOptions,
    pub workspace_id: String,
    pub revision: i64,
    pub accepted_changes: Vec<String>,
    pub excluded: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcome {
    pub path: String,
    pub package_size: u64,
    pub content_size: u64,
    pub entry_count: usize,
    pub revision: i64,
    pub excluded: usize,
    pub warnings: Vec<String>,
}

impl Store {
    /// Insert scanned items under `parent`. Name collisions are auto-numbered; sources that are
    /// already in this workspace are skipped rather than duplicated.
    ///
    /// The whole addition is one transaction: if anything fails, nothing from this call is kept.
    pub fn add_scanned(&mut self, ws: &str, parent: Option<&str>, scan: &ScanReport) -> Result<AddReport> {
        let tx = self.conn.unchecked_transaction()?;
        let rep = self.add_scanned_inner(ws, parent, scan)?; // an early return drops `tx`, rolling back
        tx.commit()?;
        Ok(rep)
    }

    fn add_scanned_inner(&self, ws: &str, parent: Option<&str>, scan: &ScanReport) -> Result<AddReport> {
        self.get_workspace(ws)?;
        let mut rep = AddReport { warnings: scan.warnings.clone(), ..Default::default() };
        let existing_sources: HashSet<String> = self.list_entries(ws)?.into_iter().filter_map(|e| e.source_path).collect();
        let by_hash: HashMap<String, String> = {
            let paths = self.entry_paths(ws)?;
            self.list_entries(ws)?
                .into_iter()
                .filter(|e| e.kind == "file")
                .filter_map(|e| e.sha256.map(|h| (h, paths[&e.id].clone())))
                .collect()
        };
        let mut seen_hash = by_hash;
        let mut folder_cache: HashMap<Vec<String>, String> = HashMap::new();
        let mut added_count = 0;
        for item in &scan.items {
            let parent_id = self.resolve_dir(ws, parent, &item.dir, &mut folder_cache)?;
            if item.is_folder {
                let key: Vec<String> = item.dir.iter().cloned().chain([item.name.clone()]).collect();
                if let std::collections::hash_map::Entry::Vacant(slot) = folder_cache.entry(key) {
                    let id = self.ensure_child_folder(ws, parent_id.as_deref(), &item.name)?;
                    slot.insert(id);
                }
                continue;
            }
            let sp = item.source_path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            if existing_sources.contains(&sp) {
                rep.already_collected.push(item.name.clone());
                continue;
            }
            let name = self.unique_name(ws, parent_id.as_deref(), &item.name, None)?;
            if name != item.name {
                rep.renamed.push((item.name.clone(), name.clone()));
            }
            self.insert_scanned_file(ws, parent_id.as_deref(), &name, item, &sp)?;
            added_count += 1;
            rep.added.push(name.clone());
            if let Some(h) = &item.sha256 {
                if item.size > 0 {
                    if let Some(existing) = seen_hash.get(h) {
                        rep.content_duplicates.push((name.clone(), existing.clone()));
                    } else {
                        seen_hash.insert(h.clone(), name);
                    }
                }
            }
        }
        self.log(
            Some(ws),
            "entry.added",
            &format!("Collected {added_count} item(s)"),
            serde_json::json!({"skipped_existing": rep.already_collected.len(), "warnings": rep.warnings.len()}),
        );
        Ok(rep)
    }

    fn resolve_dir(
        &self,
        ws: &str,
        base: Option<&str>,
        dir: &[String],
        cache: &mut HashMap<Vec<String>, String>,
    ) -> Result<Option<String>> {
        let mut cur: Option<String> = base.map(String::from);
        let mut key: Vec<String> = vec![];
        for part in dir {
            key.push(part.clone());
            if let Some(id) = cache.get(&key) {
                cur = Some(id.clone());
                continue;
            }
            let id = self.ensure_child_folder(ws, cur.as_deref(), part)?;
            cache.insert(key.clone(), id.clone());
            cur = Some(id);
        }
        Ok(cur)
    }

    fn ensure_child_folder(&self, ws: &str, parent: Option<&str>, name: &str) -> Result<String> {
        let existing = self.list_children(ws, parent)?.into_iter().find(|e| e.name.eq_ignore_ascii_case(name));
        match existing {
            Some(e) if e.kind == "folder" => Ok(e.id),
            Some(_) => Ok(self.create_folder(ws, parent, &self.unique_name(ws, parent, name, None)?)?.id),
            None => Ok(self.create_folder(ws, parent, name)?.id),
        }
    }

    fn insert_scanned_file(&self, ws: &str, parent: Option<&str>, name: &str, item: &ScannedItem, sp: &str) -> Result<()> {
        let ts = now();
        let order: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order),0)+1 FROM entries WHERE workspace_id=?1 AND parent_id IS ?2",
            params![ws, parent],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO entries(id,workspace_id,parent_id,kind,name,sort_order,origin,source_path,size,sha256,mime,source_mtime_ms,created_at,modified_at)
             VALUES(?1,?2,?3,'file',?4,?5,'source',?6,?7,?8,?9,?10,?11,?11)",
            params![new_id(), ws, parent, name, order, sp, item.size as i64, item.sha256, item.mime, item.mtime_ms, ts],
        )?;
        self.conn.execute("UPDATE workspaces SET modified_at=?2 WHERE id=?1", params![ws, ts])?;
        Ok(())
    }

    // ---------- source intelligence ----------
    pub fn source_records(&self, ws: &str) -> Result<Vec<(String, String, SourceRecord)>> {
        let paths = self.entry_paths(ws)?;
        Ok(self
            .list_entries(ws)?
            .into_iter()
            .filter(|e| e.kind == "file")
            .filter_map(|e| {
                let sp = e.source_path.clone()?;
                Some((
                    e.id.clone(),
                    paths[&e.id].clone(),
                    SourceRecord {
                        id: e.id,
                        path: PathBuf::from(sp),
                        size: e.size as u64,
                        sha256: e.sha256.unwrap_or_default(),
                        modified_ms: e.source_mtime_ms,
                    },
                ))
            })
            .collect())
    }

    pub fn check_sources(&self, ws: &str, deep: bool, cancel: &CancelToken) -> Result<Vec<SourceCheck>> {
        run_source_checks(self.source_records(ws)?, deep, cancel)
    }

    /// Point an entry at a different file. Content must match unless `accept_different` is set,
    /// in which case the new file becomes the entry's baseline (an explicit decision by the user).
    pub fn relink_source(&self, entry_id: &str, new_path: &Path, accept_different: bool) -> Result<intel::ReplacementCheck> {
        let e = self.get_entry(entry_id)?;
        if e.kind != "file" {
            return Err(WsError::Invalid("only file entries have sources".into()));
        }
        let chk = intel::validate_replacement(new_path, e.sha256.as_deref().unwrap_or(""))?;
        if !chk.identical && !accept_different {
            return Err(WsError::Invalid(
                "the replacement file's content differs from the collected file; confirm to accept the new content".into(),
            ));
        }
        self.conn.execute(
            "UPDATE entries SET source_path=?2,size=?3,sha256=?4,source_mtime_ms=?5,modified_at=?6 WHERE id=?1",
            params![entry_id, new_path.to_string_lossy(), chk.size as i64, chk.sha256, intel::mtime_ms(new_path), now()],
        )?;
        self.log(
            Some(&e.workspace_id),
            "source.relinked",
            &format!("Relinked “{}”", e.name),
            serde_json::json!({"identical": chk.identical}),
        );
        Ok(chk)
    }

    /// Re-record the current state of changed sources as the new baseline (explicit user action).
    pub fn accept_source_changes(&self, entry_ids: &[String]) -> Result<usize> {
        let mut n = 0;
        for id in entry_ids {
            let e = self.get_entry(id)?;
            let Some(sp) = e.source_path.clone() else { continue };
            let (sha, size) = parcel_core::hash::sha256_file(Path::new(&sp))?;
            self.conn.execute(
                "UPDATE entries SET size=?2,sha256=?3,source_mtime_ms=?4,modified_at=?5 WHERE id=?1",
                params![id, size as i64, sha, intel::mtime_ms(Path::new(&sp)), now()],
            )?;
            n += 1;
        }
        if n > 0 {
            if let Some(first) = entry_ids.first() {
                let ws = self.get_entry(first)?.workspace_id;
                self.log(Some(&ws), "source.accepted", &format!("Accepted changes to {n} source(s)"), serde_json::Value::Null);
            }
        }
        Ok(n)
    }

    pub fn find_duplicates(&self, ws: &str) -> Result<Vec<DuplicateGroup>> {
        let paths = self.entry_paths(ws)?;
        let items: Vec<HashedItem> = self
            .list_entries(ws)?
            .into_iter()
            .filter(|e| e.kind == "file" && e.sha256.is_some())
            .map(|e| HashedItem { label: paths[&e.id].clone(), id: e.id, size: e.size as u64, sha256: e.sha256.unwrap_or_default() })
            .collect();
        Ok(intel::find_duplicates(&items))
    }

    // ---------- export ----------
    pub fn prepare_export(&self, ws: &str, req: &ExportRequest) -> Result<PreparedExport> {
        let w = self.get_workspace(ws)?;
        let profile = self.get_profile(w.export_profile_id.as_deref())?;
        let spec: ProfileSpec = profile.spec;
        let entries = self.list_entries(ws)?;
        let paths = self.entry_paths(ws)?;

        let excluded: HashSet<String> = entries
            .iter()
            .filter(|e| e.kind == "file" && spec.exclude_patterns.iter().any(|p| glob_match(p, &e.name)))
            .map(|e| e.id.clone())
            .collect();

        // Source problems block the export unless the user accepted changes.
        let live: Vec<_> = self.source_records(ws)?.into_iter().filter(|(id, _, _)| !excluded.contains(id)).collect();
        let checks = run_source_checks(live, false, &CancelToken::new())?;
        let mut missing = vec![];
        let mut changed = vec![];
        for c in checks {
            let prob = |detail: String| SourceProblem {
                entry_id: c.entry_id.clone(),
                path: c.path.clone(),
                source_path: c.source_path.clone(),
                detail,
            };
            match &c.status {
                SourceStatus::Missing => missing.push(prob("source file not found".into())),
                SourceStatus::Unreadable { reason } => missing.push(prob(reason.clone())),
                SourceStatus::Changed { reason } => changed.push(prob(reason.clone())),
                SourceStatus::Unchanged => {}
            }
        }
        let accepted: Vec<String> = changed.iter().map(|p| p.entry_id.clone()).collect();
        if !missing.is_empty() || (!changed.is_empty() && !req.accept_changed) {
            return Err(WsError::SourceProblems { missing, changed: if req.accept_changed { vec![] } else { changed } });
        }

        let mut base = Manifest::new(&w.name, &w.description);
        base.package_id = w.package_id.clone();
        base.revision = (w.revision + 1) as u32;
        base.metadata = w.metadata.clone();
        let mut items = vec![];
        for e in &entries {
            if excluded.contains(&e.id) {
                continue;
            }
            let kind = match e.kind.as_str() {
                "folder" => EntryKind::Folder,
                "note" => EntryKind::Note,
                "link" => EntryKind::Link,
                _ => EntryKind::File,
            };
            let mut me = mf::Entry::new(kind, e.id.clone(), paths[&e.id].clone());
            me.description = e.description.clone();
            me.tags = e.tags.clone();
            me.metadata = e.metadata.clone();
            me.url = e.url.clone();
            if let Some(a) = &e.attached_to {
                if !excluded.contains(a) {
                    me.metadata.insert("parcel.attached_to".into(), a.clone());
                }
            }
            let content = match e.kind.as_str() {
                "file" => ItemContent::File(PathBuf::from(
                    e.source_path.clone().ok_or_else(|| WsError::Invalid(format!("{} has no source", e.name)))?,
                )),
                "note" => ItemContent::Bytes(e.content.clone().unwrap_or_default().into_bytes()),
                _ => ItemContent::None,
            };
            items.push(BuildItem { entry: me, content });
        }
        if spec.include_references {
            for r in self.list_references(ws)? {
                base.references.push(Reference {
                    id: r.id,
                    title: r.title,
                    url: r.url,
                    description: r.description,
                    category: r.category,
                    entry_id: r.entry_id.filter(|id| !excluded.contains(id)),
                });
            }
        }
        for d in self.list_dependencies(ws)? {
            if excluded.contains(&d.from_entry) || (d.kind == "entry" && excluded.contains(&d.target)) {
                continue;
            }
            base.dependencies.push(Dependency {
                id: d.id,
                kind: if d.kind == "entry" { DependencyKind::Entry } else { DependencyKind::External },
                from_entry: d.from_entry,
                target: d.target,
                description: d.description,
                required: d.required,
            });
        }
        let options = ExportOptions {
            overwrite: req.overwrite,
            compress: spec.compress,
            verify_after_write: spec.verify_after_write,
            limits: ResourceLimits::default(),
        };
        let notes = if spec.include_workspace_notes { w.notes.clone() } else { String::new() };
        Ok(PreparedExport {
            input: BuildInput { base, items, workspace_notes: notes, thumbnail_png: None },
            options,
            workspace_id: ws.into(),
            revision: w.revision + 1,
            accepted_changes: if req.accept_changed { accepted } else { vec![] },
            excluded: excluded.len(),
        })
    }

    pub fn finish_export(&self, prep: &PreparedExport, report: &BuildReport) -> Result<ExportOutcome> {
        let mut warnings = vec![];
        // Content that changed between the check and the read is reported, never hidden.
        for (id, _, rec) in self.source_records(&prep.workspace_id)? {
            if let Some(me) = report.manifest.entry_by_id(&id) {
                if me.sha256.as_deref().is_some_and(|h| !h.eq_ignore_ascii_case(&rec.sha256)) && !prep.accepted_changes.contains(&id) {
                    warnings.push(format!("“{}” changed while exporting; the package holds the newer content", me.path));
                }
            }
        }
        for id in &prep.accepted_changes {
            if let Some(me) = report.manifest.entry_by_id(id) {
                if let Ok(e) = self.get_entry(id) {
                    if let Some(sp) = e.source_path {
                        self.conn.execute(
                            "UPDATE entries SET size=?2,sha256=?3,source_mtime_ms=?4 WHERE id=?1",
                            params![id, me.size as i64, me.sha256, intel::mtime_ms(Path::new(&sp))],
                        )?;
                    }
                }
            }
        }
        self.conn.execute(
            "UPDATE workspaces SET revision=?2,last_export_path=?3,modified_at=?4 WHERE id=?1",
            params![prep.workspace_id, prep.revision, report.path.to_string_lossy(), now()],
        )?;
        self.conn.execute(
            "INSERT INTO revisions(workspace_id,revision,package_id,path,exported_at,entry_count,package_size) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![prep.workspace_id, prep.revision, report.manifest.package_id, report.path.to_string_lossy(), now(), report.entry_count as i64, report.package_size as i64],
        )?;
        self.add_recent_package(&report.path.to_string_lossy(), &report.manifest.name, report.package_size as i64)?;
        self.log(
            Some(&prep.workspace_id),
            "package.exported",
            &format!("Exported revision {} to {}", prep.revision, report.path.display()),
            serde_json::json!({"size": report.package_size, "entries": report.entry_count}),
        );
        Ok(ExportOutcome {
            path: report.path.to_string_lossy().into_owned(),
            package_size: report.package_size,
            content_size: report.content_size,
            entry_count: report.entry_count,
            revision: prep.revision,
            excluded: prep.excluded,
            warnings,
        })
    }

    /// Convenience used by tests and the CLI: prepare, build and finish in one call.
    pub fn export_workspace(&self, ws: &str, req: &ExportRequest, cancel: &CancelToken, progress: ProgressFn<'_>) -> Result<ExportOutcome> {
        let mut prep = self.prepare_export(ws, req)?;
        let input = std::mem::replace(
            &mut prep.input,
            BuildInput { base: Manifest::new("x", ""), items: vec![], workspace_notes: String::new(), thumbnail_png: None },
        );
        let report = parcel_core::writer::build_package(input, &req.dest, prep.options.clone(), cancel, progress)?;
        self.finish_export(&prep, &report)
    }

    // ---------- import ----------
    /// Create a new workspace from a package: stage (extract + verify) outside the database, then
    /// commit atomically. Convenience wrapper for callers that do not need to release a lock in between.
    pub fn import_package(
        &mut self,
        pkg: &Path,
        managed_dir: &Path,
        limits: &ResourceLimits,
        cancel: &CancelToken,
        progress: ProgressFn<'_>,
    ) -> Result<Workspace> {
        let staged = StagedImport::prepare(pkg, managed_dir, &new_id(), limits, cancel, progress)?;
        cancel.check()?;
        match self.commit_import(&staged) {
            Ok(w) => Ok(w),
            Err(e) => {
                staged.discard();
                Err(e)
            }
        }
    }

    // ---------- diagnostics ----------
    /// Static dependency/reference diagnostics for the whole workspace. Unlike an export preparation this
    /// never inspects source files, so it keeps working when sources are missing or changed.
    pub fn diagnose_dependencies(&self, ws: &str) -> Result<Vec<parcel_core::intel::Diagnostic>> {
        let w = self.get_workspace(ws)?;
        let mut m = Manifest::new(&w.name, "");
        let paths = self.entry_paths(ws)?;
        for e in self.list_entries(ws)? {
            let kind = match e.kind.as_str() {
                "folder" => EntryKind::Folder,
                "note" => EntryKind::Note,
                "link" => EntryKind::Link,
                _ => EntryKind::File,
            };
            m.entries.push(mf::Entry::new(kind, e.id.clone(), paths[&e.id].clone()));
        }
        for r in self.list_references(ws)? {
            m.references.push(Reference {
                id: r.id,
                title: r.title,
                url: r.url,
                description: r.description,
                category: r.category,
                entry_id: r.entry_id,
            });
        }
        for d in self.list_dependencies(ws)? {
            m.dependencies.push(Dependency {
                id: d.id,
                kind: if d.kind == "entry" { DependencyKind::Entry } else { DependencyKind::External },
                from_entry: d.from_entry,
                target: d.target,
                description: d.description,
                required: d.required,
            });
        }
        Ok(parcel_core::intel::diagnose_dependencies(&m))
    }
}

pub fn run_source_checks(records: Vec<(String, String, SourceRecord)>, deep: bool, cancel: &CancelToken) -> Result<Vec<SourceCheck>> {
    let mut out = Vec::with_capacity(records.len());
    for (id, path, rec) in records {
        cancel.check()?;
        let status = intel::check_source(&rec, deep);
        out.push(SourceCheck { entry_id: id, path, source_path: rec.path.to_string_lossy().into_owned(), status });
    }
    Ok(out)
}
