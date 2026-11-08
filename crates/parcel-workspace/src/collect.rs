//! Scanning files/folders from disk. Read-only: sources are never modified.
use crate::error::{Result, WsError};
use crate::store::validate_name;
use parcel_core::cancel::{CancelToken, Progress, ProgressFn};
use parcel_core::hash::sha256_file;
use parcel_core::intel::mtime_ms;
use parcel_core::pathsafe::sanitize_component;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct ScannedItem {
    pub is_folder: bool,
    /// Folder components (below the target parent) this item lives in.
    pub dir: Vec<String>,
    pub name: String,
    pub source_path: Option<PathBuf>,
    pub size: u64,
    pub sha256: Option<String>,
    pub mime: Option<String>,
    pub mtime_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScanReport {
    pub items: Vec<ScannedItem>,
    pub warnings: Vec<String>,
}

pub const MAX_SCAN_ITEMS: usize = 200_000;

fn display_name(p: &Path) -> String {
    let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "untitled".into());
    sanitize_component(&n)
}

pub fn scan_paths(paths: &[PathBuf], cancel: &CancelToken, progress: ProgressFn<'_>) -> Result<ScanReport> {
    let mut rep = ScanReport::default();
    for p in paths {
        scan_one(p, &[], &mut rep, cancel, progress)?;
    }
    Ok(rep)
}

fn scan_one(p: &Path, dir: &[String], rep: &mut ScanReport, cancel: &CancelToken, progress: ProgressFn<'_>) -> Result<()> {
    cancel.check()?;
    if rep.items.len() >= MAX_SCAN_ITEMS {
        return Err(WsError::Invalid(format!("more than {MAX_SCAN_ITEMS} items; add a smaller selection")));
    }
    let meta = match fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(e) => {
            rep.warnings.push(format!("cannot read {}: {e}", p.display()));
            return Ok(());
        }
    };
    if meta.file_type().is_symlink() {
        rep.warnings.push(format!("skipped symbolic link {}", p.display()));
        return Ok(());
    }
    let name = display_name(p);
    if meta.is_dir() {
        rep.items.push(ScannedItem {
            is_folder: true,
            dir: dir.to_vec(),
            name: name.clone(),
            source_path: None,
            size: 0,
            sha256: None,
            mime: None,
            mtime_ms: None,
        });
        let mut sub = dir.to_vec();
        sub.push(name);
        let mut children: Vec<PathBuf> = match fs::read_dir(p) {
            Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
            Err(e) => {
                rep.warnings.push(format!("cannot list {}: {e}", p.display()));
                return Ok(());
            }
        };
        children.sort();
        for c in children {
            scan_one(&c, &sub, rep, cancel, progress)?;
        }
    } else if meta.is_file() {
        progress(&Progress {
            phase: "scanning".into(),
            current_item: p.display().to_string(),
            items_done: rep.items.len() as u64,
            items_total: 0,
            bytes_done: 0,
            bytes_total: 0,
        });
        let hashed = (|| -> Result<(String, u64, Vec<u8>)> {
            let mut head = vec![0u8; 512];
            let n = fs::File::open(p)?.read(&mut head)?;
            head.truncate(n);
            let (sha, size) = sha256_file(p)?;
            Ok((sha, size, head))
        })();
        match hashed {
            Ok((sha, size, head)) => {
                let mime = parcel_core::mime::guess(&name, &head);
                rep.items.push(ScannedItem {
                    is_folder: false,
                    dir: dir.to_vec(),
                    name,
                    source_path: Some(p.to_path_buf()),
                    size,
                    sha256: Some(sha),
                    mime,
                    mtime_ms: mtime_ms(p),
                });
            }
            Err(e) => rep.warnings.push(format!("cannot read {}: {e}", p.display())),
        }
    } else {
        rep.warnings.push(format!("skipped special file {}", p.display()));
    }
    Ok(())
}

pub fn check_valid_names(rep: &ScanReport) -> Result<()> {
    for i in &rep.items {
        validate_name(&i.name)?;
    }
    Ok(())
}
