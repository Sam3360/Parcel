//! Tauri IPC commands. Thin adapters: validate input, call the tested crates, map errors.
use crate::error::{IpcError, IpcResult};
use crate::state::{emitter, AppState};
use base64::Engine;
use parcel_core::cancel::CancelToken;
use parcel_core::intel::{diagnose_dependencies, Diagnostic};
use parcel_core::manifest::{EntryKind, Manifest};
use parcel_core::reader::{ConflictPolicy, ExtractOptions, ExtractReport, IntegrityReport, PackageReader};
use parcel_core::writer::ExportOptions;
use parcel_workspace::collect::scan_paths;
use parcel_workspace::import::StagedImport;
use parcel_workspace::launch::strip_verbatim_prefix;
use parcel_workspace::models::*;
use parcel_workspace::ops::{AddReport, ExportOutcome, ExportRequest, SourceCheck};
use parcel_workspace::templates::{ExportProfile, ProfileSpec, Template, TemplateSpec};
use parcel_workspace::Store;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

type St<'a> = State<'a, AppState>;

fn abs(p: &str) -> IpcResult<PathBuf> {
    let pb = PathBuf::from(p);
    if !pb.is_absolute() || p.contains('\0') {
        return Err(IpcError::invalid(format!("expected an absolute path, got {p:?}")));
    }
    Ok(pb)
}

fn with_store<T>(s: &St, f: impl FnOnce(&mut Store) -> Result<T, parcel_workspace::WsError>) -> IpcResult<T> {
    let mut g = s.store.lock().map_err(|_| IpcError::invalid("internal lock poisoned"))?;
    Ok(f(&mut g)?)
}

// ---------------- workspaces ----------------
#[tauri::command]
pub fn list_workspaces(s: St, trashed: bool) -> IpcResult<Vec<WorkspaceSummary>> {
    with_store(&s, |st| st.list_workspaces(trashed))
}
#[tauri::command]
pub fn recent_workspaces(s: St, limit: i64) -> IpcResult<Vec<WorkspaceSummary>> {
    with_store(&s, |st| st.recent_workspaces(limit))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn create_workspace(s: St, name: String, description: String, templateId: Option<String>) -> IpcResult<Workspace> {
    with_store(&s, |st| st.create_workspace(&name, &description, templateId.as_deref()))
}
#[tauri::command]
pub fn get_workspace(s: St, id: String) -> IpcResult<Workspace> {
    with_store(&s, |st| {
        let w = st.get_workspace(&id)?;
        st.touch_opened(&id)?;
        Ok(w)
    })
}
#[tauri::command]
pub fn update_workspace(s: St, id: String, update: WorkspaceUpdate) -> IpcResult<Workspace> {
    with_store(&s, |st| st.update_workspace(&id, update))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn duplicate_workspace(s: St, id: String, newName: String) -> IpcResult<Workspace> {
    with_store(&s, |st| st.duplicate_workspace(&id, &newName))
}
#[tauri::command]
pub fn trash_workspace(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.trash_workspace(&id))
}
#[tauri::command]
pub fn restore_workspace(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.restore_workspace(&id))
}
#[tauri::command]
pub fn purge_workspace(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.purge_workspace(&id))?;
    cleanup_managed(&s);
    Ok(())
}

/// Remove imported-file folders nothing references any more (never ones for imports in progress).
pub fn cleanup_managed(s: &AppState) {
    let protected = s.protected_import_ids();
    if let Ok(g) = s.store.lock() {
        let _ = g.cleanup_managed_dirs(&s.managed_dir(), &protected);
    }
}

// ---------------- entries ----------------
#[tauri::command]
#[allow(non_snake_case)]
pub fn list_entries(s: St, workspaceId: String) -> IpcResult<Vec<Entry>> {
    with_store(&s, |st| st.list_entries(&workspaceId))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn create_folder(s: St, workspaceId: String, parentId: Option<String>, name: String) -> IpcResult<Entry> {
    with_store(&s, |st| st.create_folder(&workspaceId, parentId.as_deref(), &name))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn create_note(
    s: St,
    workspaceId: String,
    parentId: Option<String>,
    name: String,
    content: String,
    attachedTo: Option<String>,
) -> IpcResult<Entry> {
    with_store(&s, |st| st.create_note(&workspaceId, parentId.as_deref(), &name, &content, attachedTo.as_deref()))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn create_link(
    s: St,
    workspaceId: String,
    parentId: Option<String>,
    name: String,
    url: String,
    description: String,
) -> IpcResult<Entry> {
    with_store(&s, |st| st.create_link(&workspaceId, parentId.as_deref(), &name, &url, &description))
}
#[tauri::command]
pub fn rename_entry(s: St, id: String, name: String) -> IpcResult<Entry> {
    with_store(&s, |st| st.rename_entry(&id, &name))
}
#[tauri::command]
pub fn update_entry(s: St, id: String, update: EntryUpdate) -> IpcResult<Entry> {
    with_store(&s, |st| st.update_entry(&id, update))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn move_entries(s: St, ids: Vec<String>, newParent: Option<String>) -> IpcResult<()> {
    with_store(&s, |st| st.move_entries(&ids, newParent.as_deref()))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn reorder_entries(s: St, workspaceId: String, parentId: Option<String>, orderedIds: Vec<String>) -> IpcResult<()> {
    with_store(&s, |st| st.reorder(&workspaceId, parentId.as_deref(), &orderedIds))
}
#[tauri::command]
pub fn delete_entries(s: St, ids: Vec<String>) -> IpcResult<usize> {
    with_store(&s, |st| st.delete_entries(&ids))
}
#[tauri::command]
pub fn batch_tag(s: St, ids: Vec<String>, add: Vec<String>, remove: Vec<String>) -> IpcResult<()> {
    with_store(&s, |st| st.batch_tag(&ids, &add, &remove))
}
#[tauri::command]
pub fn batch_set_metadata(s: St, ids: Vec<String>, key: String, value: Option<String>) -> IpcResult<()> {
    with_store(&s, |st| st.batch_set_metadata(&ids, &key, value.as_deref()))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn search_entries(s: St, workspaceId: String, query: SearchQuery) -> IpcResult<Vec<Entry>> {
    with_store(&s, |st| st.search(&workspaceId, &query))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn workspace_stats(s: St, workspaceId: String) -> IpcResult<WorkspaceStats> {
    with_store(&s, |st| st.workspace_stats(&workspaceId))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn all_tags(s: St, workspaceId: String) -> IpcResult<Vec<(String, i64)>> {
    with_store(&s, |st| st.all_tags(&workspaceId))
}

// ---------------- references & dependencies ----------------
#[tauri::command]
#[allow(non_snake_case)]
pub fn list_references(s: St, workspaceId: String) -> IpcResult<Vec<RefRow>> {
    with_store(&s, |st| st.list_references(&workspaceId))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn add_reference(s: St, workspaceId: String, input: RefInput) -> IpcResult<RefRow> {
    with_store(&s, |st| st.add_reference(&workspaceId, input))
}
#[tauri::command]
pub fn update_reference(s: St, id: String, input: RefInput) -> IpcResult<RefRow> {
    with_store(&s, |st| st.update_reference(&id, input))
}
#[tauri::command]
pub fn delete_reference(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.delete_reference(&id))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn list_dependencies(s: St, workspaceId: String) -> IpcResult<Vec<DepRow>> {
    with_store(&s, |st| st.list_dependencies(&workspaceId))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn add_dependency(
    s: St,
    workspaceId: String,
    fromEntry: String,
    kind: String,
    target: String,
    description: String,
    required: bool,
) -> IpcResult<DepRow> {
    with_store(&s, |st| st.add_dependency(&workspaceId, &fromEntry, &kind, &target, &description, required))
}
#[tauri::command]
pub fn delete_dependency(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.delete_dependency(&id))
}

/// Open an http/https/ftp/mailto URL in the user's default browser. Only ever called from an explicit click.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> IpcResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !parcel_core::manifest::is_allowed_url(&url) {
        return Err(IpcError::invalid("only http, https, ftp and mailto links can be opened"));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| IpcError::invalid(e.to_string()))
}

// ---------------- collection & intelligence ----------------
#[tauri::command]
#[allow(non_snake_case)]
pub async fn add_paths(
    app: AppHandle,
    s: St<'_>,
    workspaceId: String,
    parentId: Option<String>,
    paths: Vec<String>,
    jobId: String,
) -> IpcResult<AddReport> {
    let paths: Vec<PathBuf> = paths.iter().map(|p| abs(p)).collect::<IpcResult<_>>()?;
    let cancel = s.register_job(&jobId);
    let em = emitter(app, jobId.clone());
    // Scan (hash) outside the database lock so the UI stays responsive.
    let res = tauri::async_runtime::spawn_blocking(move || scan_paths(&paths, &cancel, &em))
        .await
        .map_err(|e| IpcError::invalid(e.to_string()))?;
    s.finish_job(&jobId);
    let scan = res.map_err(IpcError::from)?;
    with_store(&s, |st| st.add_scanned(&workspaceId, parentId.as_deref(), &scan))
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn check_sources(s: St<'_>, workspaceId: String, deep: bool, jobId: String) -> IpcResult<Vec<SourceCheck>> {
    let records = with_store(&s, |st| st.source_records(&workspaceId))?;
    let cancel = s.register_job(&jobId);
    let res = tauri::async_runtime::spawn_blocking(move || parcel_workspace::ops::run_source_checks(records, deep, &cancel))
        .await
        .map_err(|e| IpcError::invalid(e.to_string()))?;
    s.finish_job(&jobId);
    Ok(res?)
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn relink_source(s: St, entryId: String, newPath: String, acceptDifferent: bool) -> IpcResult<parcel_core::intel::ReplacementCheck> {
    let p = abs(&newPath)?;
    with_store(&s, |st| st.relink_source(&entryId, &p, acceptDifferent))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn accept_source_changes(s: St, entryIds: Vec<String>) -> IpcResult<usize> {
    with_store(&s, |st| st.accept_source_changes(&entryIds))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn find_duplicates(s: St, workspaceId: String) -> IpcResult<Vec<parcel_core::intel::DuplicateGroup>> {
    with_store(&s, |st| st.find_duplicates(&workspaceId))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn workspace_diagnostics(s: St, workspaceId: String) -> IpcResult<Vec<Diagnostic>> {
    // Static analysis only: must keep working when source files are missing or changed.
    with_store(&s, |st| st.diagnose_dependencies(&workspaceId))
}

// ---------------- export / import ----------------
#[tauri::command]
#[allow(non_snake_case)]
pub async fn export_workspace(
    app: AppHandle,
    s: St<'_>,
    workspaceId: String,
    dest: String,
    overwrite: bool,
    acceptChanged: bool,
    jobId: String,
) -> IpcResult<ExportOutcome> {
    let dest = abs(&dest)?;
    let req = ExportRequest { dest: dest.clone(), overwrite, accept_changed: acceptChanged };
    let mut prep = with_store(&s, |st| st.prepare_export(&workspaceId, &req))?;
    prep.options.limits = s.limits();
    let cancel = s.register_job(&jobId);
    let em = emitter(app, jobId.clone());
    let input = std::mem::replace(
        &mut prep.input,
        parcel_core::writer::BuildInput {
            base: Manifest::new("x", ""),
            items: vec![],
            workspace_notes: String::new(),
            thumbnail_png: None,
        },
    );
    let opts = prep.options.clone();
    let report = tauri::async_runtime::spawn_blocking(move || parcel_core::writer::build_package(input, &dest, opts, &cancel, &em))
        .await
        .map_err(|e| IpcError::invalid(e.to_string()))?;
    s.finish_job(&jobId);
    let report = report?;
    s.remember_path(report.path.clone());
    with_store(&s, |st| st.finish_export(&prep, &report))
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn import_package(app: AppHandle, s: St<'_>, path: String, jobId: String) -> IpcResult<Workspace> {
    let path = abs(&path)?;
    let limits = s.limits();
    let managed = s.managed_dir();
    let cancel = s.register_job(&jobId);
    let em = emitter(app, jobId.clone());
    let store = s.store.clone();
    let ws_id = uuid::Uuid::new_v4().to_string();
    // Protect the import's folder from cleanup for as long as it is being staged or committed.
    s.staging.lock().unwrap().insert(ws_id.clone());
    let id_for_task = ws_id.clone();
    let res = tauri::async_runtime::spawn_blocking(move || -> Result<Workspace, parcel_workspace::WsError> {
        // Phase 1: extract + verify with NO database lock held; cleans up after itself on failure/cancel.
        let staged = StagedImport::prepare(&path, &managed, &id_for_task, &limits, &cancel, &em)?;
        if let Err(e) = cancel.check() {
            staged.discard();
            return Err(e.into());
        }
        // Phase 2: one short, atomic transaction.
        let mut g = store.lock().map_err(|_| parcel_workspace::WsError::Invalid("internal lock poisoned".into()))?;
        g.commit_import(&staged).inspect_err(|_| staged.discard())
    })
    .await;
    s.staging.lock().unwrap().remove(&ws_id);
    s.finish_job(&jobId);
    match res {
        Ok(r) => Ok(r?),
        Err(join_err) => {
            // The task panicked: make sure no orphaned folder is left behind.
            cleanup_managed(&s);
            Err(IpcError::invalid(join_err.to_string()))
        }
    }
}

#[derive(Serialize)]
pub struct ZipImportResult {
    pub path: String,
    pub entries: usize,
    pub size: u64,
    pub warnings: Vec<String>,
}

/// Convert a ZIP into a new `.parcel` file (never modifies the ZIP).
#[tauri::command]
#[allow(non_snake_case)]
pub async fn convert_zip(
    app: AppHandle,
    s: St<'_>,
    zipPath: String,
    dest: String,
    name: String,
    overwrite: bool,
    jobId: String,
) -> IpcResult<ZipImportResult> {
    let (zip, dest) = (abs(&zipPath)?, abs(&dest)?);
    let cancel = s.register_job(&jobId);
    let em = emitter(app, jobId.clone());
    let opts = ExportOptions { overwrite, limits: s.limits(), ..Default::default() };
    let res =
        tauri::async_runtime::spawn_blocking(move || parcel_core::zipimport::convert_zip_to_parcel(&zip, &dest, &name, opts, &cancel, &em))
            .await
            .map_err(|e| IpcError::invalid(e.to_string()))?;
    s.finish_job(&jobId);
    let r = res?;
    s.remember_path(r.build.path.clone());
    with_store(&s, |st| st.add_recent_package(&r.build.path.to_string_lossy(), &r.build.manifest.name, r.build.package_size as i64))?;
    Ok(ZipImportResult {
        path: r.build.path.to_string_lossy().into(),
        entries: r.build.entry_count,
        size: r.build.package_size,
        warnings: r.warnings,
    })
}

// ---------------- package explorer ----------------
#[derive(Serialize)]
pub struct PackageInfo {
    pub path: String,
    pub file_size: u64,
    pub manifest: Manifest,
    pub notes: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[tauri::command]
pub fn open_package(s: St, path: String) -> IpcResult<PackageInfo> {
    let p = abs(&path)?;
    let mut r = PackageReader::open(&p, &s.limits())?;
    s.remember_path(p.clone());
    let notes = r.notes()?;
    let m = r.manifest().clone();
    let size = r.file_size();
    with_store(&s, |st| st.add_recent_package(&p.to_string_lossy(), &m.name, size as i64))?;
    let diagnostics = diagnose_dependencies(&m);
    Ok(PackageInfo { path: path.clone(), file_size: size, manifest: m, notes, diagnostics })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn verify_package(app: AppHandle, s: St<'_>, path: String, jobId: String) -> IpcResult<IntegrityReport> {
    let p = abs(&path)?;
    let limits = s.limits();
    let cancel = s.register_job(&jobId);
    let em = emitter(app, jobId.clone());
    let res = tauri::async_runtime::spawn_blocking(move || -> Result<IntegrityReport, parcel_core::ParcelError> {
        PackageReader::open(&p, &limits)?.verify(&cancel, &em)
    })
    .await
    .map_err(|e| IpcError::invalid(e.to_string()))?;
    s.finish_job(&jobId);
    let rep = res?;
    with_store(&s, |st| {
        st.log(
            None,
            if rep.ok { "package.verified" } else { "package.verify_failed" },
            &format!("Verified {} — {}", path, if rep.ok { "passed" } else { "FAILED" }),
            serde_json::json!({"failures": rep.failures.len()}),
        );
        Ok(())
    })?;
    Ok(rep)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn extract_package(
    app: AppHandle,
    s: St<'_>,
    path: String,
    dest: String,
    ids: Option<Vec<String>>,
    conflict: ConflictPolicy,
    jobId: String,
) -> IpcResult<ExtractReport> {
    let (p, d) = (abs(&path)?, abs(&dest)?);
    let dest_for_reveal = d.clone();
    let limits = s.limits();
    let cancel = s.register_job(&jobId);
    let em = emitter(app, jobId.clone());
    let res = tauri::async_runtime::spawn_blocking(move || -> Result<ExtractReport, parcel_core::ParcelError> {
        let mut r = PackageReader::open(&p, &limits)?;
        r.extract(&d, &ExtractOptions { selection: ids, conflict, limits }, &cancel, &em)
    })
    .await
    .map_err(|e| IpcError::invalid(e.to_string()))?;
    s.finish_job(&jobId);
    let rep = res?;
    s.remember_path(dest_for_reveal);
    with_store(&s, |st| {
        st.log(None, "package.extracted", &format!("Extracted {} file(s) from {}", rep.written.len(), path), serde_json::Value::Null);
        Ok(())
    })?;
    Ok(rep)
}

#[derive(Serialize, Default)]
pub struct Preview {
    pub kind: String, // text | markdown | json | csv | image | pdf | none
    pub mime: Option<String>,
    pub text: Option<String>,
    pub data_base64: Option<String>,
    pub truncated: bool,
    pub reason: Option<String>,
}

const TEXT_LIMIT: u64 = 1024 * 1024;
const IMAGE_LIMIT: u64 = 12 * 1024 * 1024;

fn classify(name: &str, mime: Option<&str>) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    match (mime.unwrap_or(""), ext.as_str()) {
        ("image/svg+xml", _) | (_, "svg") => "image",
        (m, _) if m.starts_with("image/") && m != "image/bmp" => "image",
        (_, "bmp") => "image",
        ("application/pdf", _) | (_, "pdf") => "pdf",
        (_, "md" | "markdown") => "markdown",
        ("application/json", _) | (_, "json") => "json",
        ("text/csv", _) | (_, "csv" | "tsv") => "csv",
        (m, _) if m.starts_with("text/") => "text",
        (
            _,
            "txt" | "log" | "rs" | "ts" | "tsx" | "js" | "py" | "toml" | "yaml" | "yml" | "xml" | "ini" | "cfg" | "sh" | "ps1" | "css"
            | "html" | "htm" | "c" | "h" | "cpp" | "java" | "go",
        ) => "text",
        _ => "none",
    }
}

/// Build a preview from bytes. Content is returned as inert data; HTML/SVG/scripts are never executed or injected.
fn make_preview(name: &str, mime: Option<&str>, bytes: Vec<u8>, truncated: bool) -> Preview {
    let kind = classify(name, mime);
    match kind {
        "image" | "pdf" => {
            let m =
                if kind == "pdf" { "application/pdf".to_string() } else { mime.map(String::from).unwrap_or_else(|| "image/png".into()) };
            Preview {
                kind: kind.into(),
                mime: Some(m),
                data_base64: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
                truncated,
                ..Default::default()
            }
        }
        "none" => Preview { kind: "none".into(), reason: Some("No preview is available for this file type.".into()), ..Default::default() },
        _ => {
            if bytes.contains(&0) {
                return Preview { kind: "none".into(), reason: Some("This file looks binary.".into()), ..Default::default() };
            }
            let mut text = String::from_utf8_lossy(&bytes).into_owned();
            if kind == "json" && !truncated {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    text = serde_json::to_string_pretty(&v).unwrap_or(text);
                }
            }
            Preview { kind: kind.into(), mime: mime.map(String::from), text: Some(text), truncated, ..Default::default() }
        }
    }
}

fn read_limited(path: &Path, limit: u64) -> std::io::Result<(Vec<u8>, bool)> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)?.take(limit + 1).read_to_end(&mut buf)?;
    let truncated = buf.len() as u64 > limit;
    buf.truncate(limit as usize);
    Ok((buf, truncated))
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn preview_package_entry(s: St, path: String, entryId: String) -> IpcResult<Preview> {
    let p = abs(&path)?;
    let mut r = PackageReader::open(&p, &s.limits())?;
    let e = r.manifest().entry_by_id(&entryId).cloned().ok_or_else(|| IpcError::invalid("unknown entry"))?;
    if !matches!(e.kind, EntryKind::File | EntryKind::Note) {
        return Ok(Preview { kind: "none".into(), reason: Some("Nothing to preview for this item.".into()), ..Default::default() });
    }
    let kind = classify(&e.path, e.mime.as_deref());
    let limit = if matches!(kind, "image" | "pdf") { IMAGE_LIMIT } else { TEXT_LIMIT };
    if kind == "none" {
        return Ok(make_preview(&e.path, e.mime.as_deref(), vec![], false));
    }
    if e.size <= limit {
        // Whole file: read with checksum verification.
        let mut buf = Vec::new();
        r.read_entry_verified(&entryId, &mut buf, &CancelToken::new())?;
        Ok(make_preview(&e.path, e.mime.as_deref(), buf, false))
    } else if matches!(kind, "image" | "pdf") {
        Ok(Preview { kind: "none".into(), reason: Some("File is too large to preview.".into()), ..Default::default() })
    } else {
        let (b, t) = r.read_entry_prefix(&entryId, limit)?;
        Ok(make_preview(&e.path, e.mime.as_deref(), b, t))
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn preview_entry(s: St, entryId: String) -> IpcResult<Preview> {
    let e = with_store(&s, |st| st.get_entry(&entryId))?;
    match e.kind.as_str() {
        "note" => Ok(make_preview(&e.name, Some("text/markdown"), e.content.unwrap_or_default().into_bytes(), false)),
        "file" => {
            let Some(sp) = e.source_path else {
                return Ok(Preview { kind: "none".into(), reason: Some("No source recorded.".into()), ..Default::default() });
            };
            let kind = classify(&e.name, e.mime.as_deref());
            let limit = if matches!(kind, "image" | "pdf") { IMAGE_LIMIT } else { TEXT_LIMIT };
            if kind == "none" {
                return Ok(make_preview(&e.name, e.mime.as_deref(), vec![], false));
            }
            match read_limited(Path::new(&sp), limit) {
                Ok((b, t)) if !(t && matches!(kind, "image" | "pdf")) => Ok(make_preview(&e.name, e.mime.as_deref(), b, t)),
                Ok(_) => Ok(Preview { kind: "none".into(), reason: Some("File is too large to preview.".into()), ..Default::default() }),
                Err(err) => Ok(Preview { kind: "none".into(), reason: Some(format!("Source unavailable: {err}")), ..Default::default() }),
            }
        }
        _ => Ok(Preview { kind: "none".into(), reason: Some("Nothing to preview.".into()), ..Default::default() }),
    }
}

#[tauri::command]
pub fn compare_packages(s: St, old: String, new: String) -> IpcResult<parcel_core::compare::Comparison> {
    let (a, b) = (abs(&old)?, abs(&new)?);
    let l = s.limits();
    let ra = PackageReader::open(&a, &l)?;
    let rb = PackageReader::open(&b, &l)?;
    Ok(parcel_core::compare::compare_manifests(ra.manifest(), rb.manifest()))
}

// ---------------- templates / profiles ----------------
#[tauri::command]
pub fn list_templates(s: St) -> IpcResult<Vec<Template>> {
    with_store(&s, |st| st.list_templates())
}
#[tauri::command]
pub fn save_template(s: St, id: Option<String>, name: String, description: String, spec: TemplateSpec) -> IpcResult<Template> {
    with_store(&s, |st| st.save_template(id.as_deref(), &name, &description, &spec))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn template_from_workspace(s: St, workspaceId: String, name: String, description: String, includeNotes: bool) -> IpcResult<Template> {
    with_store(&s, |st| st.template_from_workspace(&workspaceId, &name, &description, includeNotes))
}
#[tauri::command]
pub fn delete_template(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.delete_template(&id))
}
#[tauri::command]
pub fn list_profiles(s: St) -> IpcResult<Vec<ExportProfile>> {
    with_store(&s, |st| st.list_profiles())
}
#[tauri::command]
pub fn save_profile(s: St, id: Option<String>, name: String, description: String, spec: ProfileSpec) -> IpcResult<ExportProfile> {
    with_store(&s, |st| st.save_profile(id.as_deref(), &name, &description, &spec))
}
#[tauri::command]
pub fn delete_profile(s: St, id: String) -> IpcResult<()> {
    with_store(&s, |st| st.delete_profile(&id))
}

// ---------------- opening packages from outside / file manager ----------------
/// Hand the UI any package paths passed on the command line or by a second instance (drains the queue).
#[tauri::command]
pub fn take_launch_packages(s: St) -> Vec<String> {
    std::mem::take(&mut *s.pending_open.lock().unwrap())
}

fn reveal(app: &AppHandle, p: &Path) -> IpcResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let p = strip_verbatim_prefix(p);
    let kind = std::fs::symlink_metadata(&p).ok();
    let err = |e: tauri_plugin_opener::Error| IpcError::invalid(e.to_string());
    match kind {
        Some(m) if m.is_file() => app.opener().reveal_item_in_dir(&p).map_err(err),
        Some(m) if m.is_dir() => app.opener().open_path(p.to_string_lossy(), None::<&str>).map_err(err),
        _ => match p.parent() {
            // The file is gone: show the folder it used to live in, if that still exists.
            Some(dir) if dir.is_dir() => app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(err),
            _ => Err(IpcError::invalid("that location no longer exists")),
        },
    }
}

/// Show a collected file's recorded source location in the file manager. The path comes from the database,
/// never from the caller.
#[tauri::command]
#[allow(non_snake_case)]
pub fn reveal_source(app: AppHandle, s: St, entryId: String) -> IpcResult<()> {
    let e = with_store(&s, |st| st.get_entry(&entryId))?;
    match (e.kind.as_str(), e.source_path) {
        ("file", Some(sp)) => reveal(&app, Path::new(&sp)),
        _ => Err(IpcError::invalid("this item has no source file")),
    }
}

/// Show a package/folder that this app just created or opened. Arbitrary paths are refused.
#[tauri::command]
pub fn reveal_output(app: AppHandle, s: St, path: String) -> IpcResult<()> {
    let p = abs(&path)?;
    if !s.is_known_path(&p) {
        return Err(IpcError::invalid("only locations created or opened by Parcel can be shown"));
    }
    reveal(&app, &p)
}

// ---------------- activity / recents / settings / jobs ----------------
#[tauri::command]
#[allow(non_snake_case)]
pub fn list_activity(s: St, workspaceId: Option<String>, limit: i64) -> IpcResult<Vec<ActivityRow>> {
    with_store(&s, |st| st.list_activity(workspaceId.as_deref(), limit))
}
#[tauri::command]
pub fn clear_activity(s: St) -> IpcResult<()> {
    with_store(&s, |st| st.clear_activity())
}
#[tauri::command]
pub fn list_recent_packages(s: St, limit: i64) -> IpcResult<Vec<RecentPackage>> {
    with_store(&s, |st| st.list_recent_packages(limit))
}
#[tauri::command]
pub fn remove_recent_package(s: St, path: String) -> IpcResult<()> {
    with_store(&s, |st| st.remove_recent_package(&path))
}
#[tauri::command]
pub fn clear_recent_packages(s: St) -> IpcResult<()> {
    with_store(&s, |st| st.clear_recent_packages())
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn list_revisions(s: St, workspaceId: String) -> IpcResult<Vec<RevisionRow>> {
    with_store(&s, |st| st.list_revisions(&workspaceId))
}
#[tauri::command]
pub fn get_prefs(s: St) -> IpcResult<BTreeMap<String, String>> {
    with_store(&s, |st| st.all_prefs())
}
#[tauri::command]
pub fn set_pref(s: St, key: String, value: String) -> IpcResult<()> {
    const ALLOWED: &[&str] = &["theme", "view", "density", "confirm_remove", "limits", "sort", "reduce_motion"];
    if !ALLOWED.contains(&key.as_str()) {
        return Err(IpcError::invalid(format!("unknown setting {key:?}")));
    }
    if key == "limits" {
        serde_json::from_str::<parcel_core::limits::ResourceLimits>(&value)?;
    }
    with_store(&s, |st| st.set_pref(&key, &value))
}
#[tauri::command]
#[allow(non_snake_case)]
pub fn cancel_job(s: St, jobId: String) -> IpcResult<bool> {
    Ok(s.jobs.lock().unwrap().get(&jobId).map(|t| t.cancel()).is_some())
}
#[tauri::command]
pub fn app_info(s: St) -> IpcResult<serde_json::Value> {
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "data_dir": s.data_dir.to_string_lossy(),
        "schema_version": with_store(&s, |st| st.schema_version())?,
        "format_version": parcel_core::manifest::FORMAT_VERSION,
    }))
}
