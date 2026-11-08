//! Reading, verifying and extracting packages. Every archive is treated as hostile input.
use crate::cancel::{CancelToken, Progress, ProgressFn};
use crate::error::{ParcelError, Result};
use crate::hash::BUF;
use crate::limits::ResourceLimits;
use crate::manifest::*;
use crate::pathsafe::{collision_key, safe_join, validate_logical_path};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Missing,
    SizeMismatch { expected: u64, actual: u64 },
    HashMismatch,
    Unreadable { reason: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryCheck {
    pub id: String,
    pub path: String,
    #[serde(flatten)]
    pub result: CheckStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntegrityReport {
    pub ok: bool,
    pub checked: usize,
    pub failures: Vec<EntryCheck>,
    pub metadata_failures: Vec<String>,
    /// Archive members present but not described by the manifest.
    pub unlisted: Vec<String>,
    pub warnings: Vec<String>,
}

impl IntegrityReport {
    pub fn to_text(&self) -> String {
        let mut s = format!("Integrity report: {}\nEntries checked: {}\n", if self.ok { "PASSED" } else { "FAILED" }, self.checked);
        for f in &self.failures {
            s += &format!("  FAIL {} — {:?}\n", f.path, f.result);
        }
        for m in &self.metadata_failures {
            s += &format!("  FAIL metadata {m}\n");
        }
        for u in &self.unlisted {
            s += &format!("  WARN unlisted archive member {u}\n");
        }
        for w in &self.warnings {
            s += &format!("  WARN {w}\n");
        }
        s
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Abort before writing anything if a destination already exists (default).
    Fail,
    Skip,
    /// Write alongside the existing file as `name (1).ext`.
    Rename,
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Entry ids to extract; selecting a folder includes everything beneath it. `None` = everything.
    pub selection: Option<Vec<String>>,
    pub conflict: ConflictPolicy,
    pub limits: ResourceLimits,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self { selection: None, conflict: ConflictPolicy::Fail, limits: ResourceLimits::default() }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExtractReport {
    pub written: Vec<PathBuf>,
    pub skipped: Vec<String>,
    pub bytes_written: u64,
}

/// Result of scanning an archive's central directory before trusting any of it.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub names: HashMap<String, usize>,
    pub warnings: Vec<String>,
    pub total_uncompressed: u64,
}

pub fn is_symlink_mode(mode: Option<u32>) -> bool {
    mode.is_some_and(|m| m & 0o170000 == 0o120000)
}

/// Scan central directory metadata only (nothing is decompressed).
/// `strict`: symlinks and unsafe names abort. Non-strict skips symlinks with a warning (ZIP import).
pub fn scan_archive<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>, limits: &ResourceLimits, strict: bool) -> Result<ScanResult> {
    if archive.len() > limits.max_entries + 8 {
        return Err(ParcelError::LimitExceeded(format!("archive has {} members", archive.len())));
    }
    let mut out = ScanResult::default();
    let mut seen = HashSet::new();
    for i in 0..archive.len() {
        let f = archive.by_index_raw(i)?;
        let raw = f.name().to_string();
        let name = if strict { raw.clone() } else { raw.replace('\\', "/") };
        if f.encrypted() {
            return Err(ParcelError::NotAPackage(format!("encrypted member {raw:?} is not supported")));
        }
        if is_symlink_mode(f.unix_mode()) {
            if strict {
                return Err(ParcelError::UnsafePath(format!("{raw:?}: symbolic links are not allowed")));
            }
            out.warnings.push(format!("skipped symbolic link {raw:?}"));
            continue;
        }
        validate_logical_path(&name, limits)?;
        if !seen.insert(collision_key(&name)) {
            return Err(ParcelError::InvalidManifest(format!("conflicting duplicate archive path {raw:?}")));
        }
        let size = f.size();
        let csize = f.compressed_size();
        if size > limits.max_entry_uncompressed {
            return Err(ParcelError::LimitExceeded(format!("{raw:?} declares {size} bytes")));
        }
        if size > 1024 * 1024 && (csize == 0 || size / csize.max(1) > limits.max_compression_ratio) {
            return Err(ParcelError::LimitExceeded(format!("{raw:?} has suspicious compression ratio (possible ZIP bomb)")));
        }
        out.total_uncompressed = out.total_uncompressed.saturating_add(size);
        if out.total_uncompressed > limits.max_total_uncompressed {
            return Err(ParcelError::LimitExceeded("archive total uncompressed size exceeds limit".into()));
        }
        out.names.insert(name, i);
    }
    Ok(out)
}

pub struct PackageReader {
    archive: ZipArchive<File>,
    manifest: Manifest,
    index: HashMap<String, usize>,
    scan_warnings: Vec<String>,
    path: PathBuf,
}

impl PackageReader {
    pub fn open(path: &Path, limits: &ResourceLimits) -> Result<Self> {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file).map_err(|e| ParcelError::NotAPackage(format!("not a readable ZIP archive: {e}")))?;
        let scan = scan_archive(&mut archive, limits, true)?;
        let mi = *scan.names.get(MANIFEST_PATH).ok_or_else(|| ParcelError::NotAPackage("manifest.json is missing".into()))?;
        let mut bytes = Vec::new();
        {
            let f = archive.by_index(mi)?;
            f.take(limits.max_manifest_bytes + 1).read_to_end(&mut bytes)?;
        }
        let manifest = parse_manifest(&bytes, limits)?;
        Ok(Self { archive, manifest, index: scan.names, scan_warnings: scan.warnings, path: path.to_path_buf() })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn file_size(&self) -> u64 {
        fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0)
    }

    pub fn notes(&mut self) -> Result<String> {
        let b = self.read_member(NOTES_PATH, 16 * 1024 * 1024)?;
        Ok(b.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default())
    }

    pub fn thumbnail(&mut self) -> Result<Option<Vec<u8>>> {
        self.read_member(THUMBNAIL_PATH, 8 * 1024 * 1024)
    }

    fn read_member(&mut self, name: &str, max: u64) -> Result<Option<Vec<u8>>> {
        let Some(&i) = self.index.get(name) else { return Ok(None) };
        let mut buf = Vec::new();
        self.archive.by_index(i)?.take(max + 1).read_to_end(&mut buf)?;
        if buf.len() as u64 > max {
            return Err(ParcelError::LimitExceeded(format!("{name} too large")));
        }
        Ok(Some(buf))
    }

    /// Read up to `max` bytes of an entry for preview. Does not hash a truncated read.
    pub fn read_entry_prefix(&mut self, id: &str, max: u64) -> Result<(Vec<u8>, bool)> {
        let e = self.manifest.entry_by_id(id).cloned().ok_or_else(|| ParcelError::Other("unknown entry".into()))?;
        let sp = e.storage_path.ok_or_else(|| ParcelError::Other("entry has no stored content".into()))?;
        let i = *self.index.get(&sp).ok_or_else(|| ParcelError::Other(format!("{sp} missing from archive")))?;
        let mut buf = Vec::new();
        self.archive.by_index(i)?.take(max).read_to_end(&mut buf)?;
        Ok((buf.clone(), (buf.len() as u64) < e.size))
    }

    /// Stream an entry to `out`, verifying size and checksum. Returns an error on mismatch;
    /// callers writing to disk must discard `out` on error.
    pub fn read_entry_verified(&mut self, id: &str, out: &mut dyn Write, cancel: &CancelToken) -> Result<u64> {
        let e = self.manifest.entry_by_id(id).cloned().ok_or_else(|| ParcelError::Other("unknown entry".into()))?;
        let sp = e.storage_path.clone().ok_or_else(|| ParcelError::Other("entry has no stored content".into()))?;
        let i = *self.index.get(&sp).ok_or_else(|| ParcelError::Other(format!("{sp} missing from archive")))?;
        let f = self.archive.by_index(i)?;
        let mut r = f.take(e.size + 1);
        let mut h = Sha256::new();
        let mut buf = vec![0u8; BUF];
        let mut total = 0u64;
        loop {
            cancel.check()?;
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > e.size {
                return Err(ParcelError::LimitExceeded(format!("{:?} is larger than declared", e.path)));
            }
            h.update(&buf[..n]);
            out.write_all(&buf[..n])?;
        }
        if total != e.size {
            return Err(ParcelError::Other(format!("{:?}: size mismatch", e.path)));
        }
        if hex::encode(h.finalize()) != e.sha256.clone().unwrap_or_default().to_lowercase() {
            return Err(ParcelError::Other(format!("{:?}: checksum mismatch", e.path)));
        }
        Ok(total)
    }

    pub fn verify(&self, cancel: &CancelToken, progress: ProgressFn<'_>) -> Result<IntegrityReport> {
        // Reopen so verification never disturbs another read in progress.
        let file = File::open(&self.path)?;
        let mut archive = ZipArchive::new(file)?;
        let mut failures = Vec::new();
        let mut checked = 0usize;
        let hashed: Vec<&Entry> = self.manifest.entries.iter().filter(|e| e.storage_path.is_some()).collect();
        let bytes_total: u64 = hashed.iter().map(|e| e.size).sum();
        let mut bytes_done = 0u64;
        for (n, e) in hashed.iter().enumerate() {
            cancel.check()?;
            progress(&Progress {
                phase: "verifying".into(),
                current_item: e.path.clone(),
                items_done: n as u64,
                items_total: hashed.len() as u64,
                bytes_done,
                bytes_total,
            });
            checked += 1;
            let sp = e.storage_path.as_deref().unwrap_or("");
            let result = match self.index.get(sp) {
                None => CheckStatus::Missing,
                Some(&i) => match archive.by_index(i) {
                    Err(err) => CheckStatus::Unreadable { reason: err.to_string() },
                    Ok(f) => {
                        let mut r = f.take(e.size + 1);
                        let mut h = Sha256::new();
                        let mut buf = vec![0u8; BUF];
                        let mut total = 0u64;
                        let mut err: Option<String> = None;
                        loop {
                            if cancel.is_cancelled() {
                                return Err(ParcelError::Cancelled);
                            }
                            match r.read(&mut buf) {
                                Ok(0) => break,
                                Ok(k) => {
                                    total += k as u64;
                                    h.update(&buf[..k]);
                                }
                                Err(x) => {
                                    err = Some(x.to_string());
                                    break;
                                }
                            }
                        }
                        bytes_done += total;
                        if let Some(reason) = err {
                            CheckStatus::Unreadable { reason }
                        } else if total != e.size {
                            CheckStatus::SizeMismatch { expected: e.size, actual: total }
                        } else if hex::encode(h.finalize()) != e.sha256.clone().unwrap_or_default().to_lowercase() {
                            CheckStatus::HashMismatch
                        } else {
                            CheckStatus::Ok
                        }
                    }
                },
            };
            if result != CheckStatus::Ok {
                failures.push(EntryCheck { id: e.id.clone(), path: e.path.clone(), result });
            }
        }
        let mut metadata_failures = Vec::new();
        for mf in &self.manifest.metadata_files {
            match self.index.get(&mf.path) {
                None => metadata_failures.push(format!("{} is missing", mf.path)),
                Some(&i) => {
                    let mut buf = Vec::new();
                    match archive.by_index(i).and_then(|f| f.take(mf.size + 1).read_to_end(&mut buf).map_err(zip::result::ZipError::Io)) {
                        Err(e) => metadata_failures.push(format!("{}: {e}", mf.path)),
                        Ok(_) => {
                            if buf.len() as u64 != mf.size || crate::hash::sha256_bytes(&buf) != mf.sha256.to_lowercase() {
                                metadata_failures.push(format!("{} does not match its checksum", mf.path));
                            }
                        }
                    }
                }
            }
        }
        let mut known: HashSet<String> = HashSet::new();
        known.insert(MANIFEST_PATH.into());
        known.extend(self.manifest.entries.iter().filter_map(|e| e.storage_path.clone()));
        known.extend(self.manifest.metadata_files.iter().map(|m| m.path.clone()));
        let mut unlisted: Vec<String> = self.index.keys().filter(|n| !n.ends_with('/') && !known.contains(*n)).cloned().collect();
        unlisted.sort();
        progress(&Progress {
            phase: "verifying".into(),
            current_item: String::new(),
            items_done: hashed.len() as u64,
            items_total: hashed.len() as u64,
            bytes_done: bytes_total,
            bytes_total,
        });
        Ok(IntegrityReport {
            ok: failures.is_empty() && metadata_failures.is_empty(),
            checked,
            failures,
            metadata_failures,
            unlisted,
            warnings: self.scan_warnings.clone(),
        })
    }

    pub fn extract(&mut self, dest: &Path, opts: &ExtractOptions, cancel: &CancelToken, progress: ProgressFn<'_>) -> Result<ExtractReport> {
        let selected = self.select_entries(opts.selection.as_deref())?;
        let total_bytes: u64 = selected.iter().map(|e| e.size).sum();
        if total_bytes > opts.limits.max_total_uncompressed {
            return Err(ParcelError::LimitExceeded("selection exceeds extraction size limit".into()));
        }
        fs::create_dir_all(dest)?;
        let dest = dest.canonicalize()?;
        // Preflight: with the Fail policy, refuse before writing anything.
        let mut plan = Vec::new();
        for e in &selected {
            let rel = target_rel_path(e);
            let target = safe_join(&dest, &rel, &opts.limits)?;
            if e.kind != EntryKind::Folder && target.exists() && opts.conflict == ConflictPolicy::Fail {
                return Err(ParcelError::AlreadyExists(target.display().to_string()));
            }
            plan.push(((*e).clone(), target));
        }
        let mut report = ExtractReport::default();
        let mut done_bytes = 0u64;
        for (n, (e, target)) in plan.iter().enumerate() {
            cancel.check()?;
            progress(&Progress {
                phase: "extracting".into(),
                current_item: e.path.clone(),
                items_done: n as u64,
                items_total: plan.len() as u64,
                bytes_done: done_bytes,
                bytes_total: total_bytes,
            });
            match e.kind {
                EntryKind::Folder => {
                    make_dirs_no_symlinks(&dest, target)?;
                }
                EntryKind::Link => {
                    let body = format!(
                        "{}\n{}\n\n{}\n",
                        e.path.rsplit('/').next().unwrap_or(&e.path),
                        e.url.clone().unwrap_or_default(),
                        e.description
                    );
                    match write_new_file(&dest, target, opts.conflict, &mut body.as_bytes())? {
                        Some(p) => {
                            report.bytes_written += body.len() as u64;
                            report.written.push(p)
                        }
                        None => report.skipped.push(e.path.clone()),
                    }
                }
                EntryKind::File | EntryKind::Note => {
                    let tmp_name = format!(".{}.parcel-part", uuid::Uuid::new_v4().simple());
                    let parent = target.parent().unwrap_or(&dest).to_path_buf();
                    make_dirs_no_symlinks(&dest, &parent)?;
                    let tmp = parent.join(tmp_name);
                    let res = (|| -> Result<u64> {
                        let mut f = File::options().write(true).create_new(true).open(&tmp)?;
                        let n = self.read_entry_verified(&e.id, &mut f, cancel)?;
                        f.sync_all()?;
                        Ok(n)
                    })();
                    match res {
                        Err(err) => {
                            let _ = fs::remove_file(&tmp);
                            return Err(err);
                        }
                        Ok(n) => {
                            match place_file(&dest, target, &tmp, opts.conflict)? {
                                Some(p) => {
                                    report.bytes_written += n;
                                    report.written.push(p);
                                }
                                None => {
                                    let _ = fs::remove_file(&tmp);
                                    report.skipped.push(e.path.clone());
                                }
                            }
                            done_bytes += n;
                        }
                    }
                }
            }
        }
        Ok(report)
    }

    fn select_entries(&self, selection: Option<&[String]>) -> Result<Vec<&Entry>> {
        let Some(sel) = selection else { return Ok(self.manifest.entries.iter().collect()) };
        let mut prefixes: Vec<String> = vec![];
        let mut ids = HashSet::new();
        for id in sel {
            let e = self.manifest.entry_by_id(id).ok_or_else(|| ParcelError::Other(format!("unknown entry id {id}")))?;
            ids.insert(e.id.clone());
            if e.kind == EntryKind::Folder {
                prefixes.push(format!("{}/", collision_key(&e.path)));
            }
        }
        Ok(self
            .manifest
            .entries
            .iter()
            .filter(|e| ids.contains(&e.id) || prefixes.iter().any(|p| collision_key(&e.path).starts_with(p)))
            .collect())
    }
}

fn target_rel_path(e: &Entry) -> String {
    match e.kind {
        EntryKind::Link => format!("{}.url.txt", e.path),
        _ => e.path.clone(),
    }
}

/// Create directories one level at a time, refusing to traverse pre-existing symlinks.
fn make_dirs_no_symlinks(root: &Path, dir: &Path) -> Result<()> {
    let rel = dir.strip_prefix(root).map_err(|_| ParcelError::UnsafePath(dir.display().to_string()))?;
    let mut cur = root.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        match fs::symlink_metadata(&cur) {
            Ok(m) if m.file_type().is_symlink() => return Err(ParcelError::UnsafePath(format!("{} is a symbolic link", cur.display()))),
            Ok(m) if !m.is_dir() => return Err(ParcelError::AlreadyExists(format!("{} exists and is not a folder", cur.display()))),
            Ok(_) => {}
            Err(_) => fs::create_dir(&cur)?,
        }
    }
    Ok(())
}

fn renamed_candidate(target: &Path, n: u32) -> PathBuf {
    let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let name = match target.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem} ({n}).{ext}"),
        None => format!("{stem} ({n})"),
    };
    target.with_file_name(name)
}

/// Atomically move `tmp` to `target` without ever replacing an existing file.
fn place_file(root: &Path, target: &Path, tmp: &Path, policy: ConflictPolicy) -> Result<Option<PathBuf>> {
    make_dirs_no_symlinks(root, target.parent().unwrap_or(root))?;
    let mut candidate = target.to_path_buf();
    let mut n = 0u32;
    loop {
        // create_new reserves the name atomically; rename then replaces only our placeholder.
        match File::options().write(true).create_new(true).open(&candidate) {
            Ok(f) => {
                drop(f);
                if let Err(e) = fs::rename(tmp, &candidate) {
                    let _ = fs::remove_file(&candidate);
                    return Err(e.into());
                }
                return Ok(Some(candidate));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => match policy {
                ConflictPolicy::Fail => return Err(ParcelError::AlreadyExists(candidate.display().to_string())),
                ConflictPolicy::Skip => return Ok(None),
                ConflictPolicy::Rename => {
                    n += 1;
                    if n > 10_000 {
                        return Err(ParcelError::Other("could not find a free file name".into()));
                    }
                    candidate = renamed_candidate(target, n);
                }
            },
            Err(e) => return Err(e.into()),
        }
    }
}

fn write_new_file(root: &Path, target: &Path, policy: ConflictPolicy, data: &mut dyn Read) -> Result<Option<PathBuf>> {
    let parent = target.parent().unwrap_or(root);
    make_dirs_no_symlinks(root, parent)?;
    let tmp = parent.join(format!(".{}.parcel-part", uuid::Uuid::new_v4().simple()));
    {
        let mut f = File::options().write(true).create_new(true).open(&tmp)?;
        std::io::copy(data, &mut f)?;
        f.sync_all()?;
    }
    match place_file(root, target, &tmp, policy) {
        Ok(Some(p)) => Ok(Some(p)),
        Ok(None) => {
            let _ = fs::remove_file(&tmp);
            Ok(None)
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}
