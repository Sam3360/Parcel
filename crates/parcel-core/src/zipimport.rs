//! Convert an arbitrary (untrusted) ZIP archive into a valid `.parcel` package.
use crate::cancel::{CancelToken, ProgressFn};
use crate::error::{ParcelError, Result};
use crate::manifest::*;
use crate::pathsafe::collision_key;
use crate::reader::scan_archive;
use crate::writer::{BuildReport, ExportOptions, PackageWriter};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct ZipImportReport {
    pub build: BuildReport,
    pub warnings: Vec<String>,
}

pub fn convert_zip_to_parcel(
    zip_path: &Path,
    dest: &Path,
    name: &str,
    opts: ExportOptions,
    cancel: &CancelToken,
    progress: ProgressFn<'_>,
) -> Result<ZipImportReport> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| ParcelError::NotAPackage(format!("not a readable ZIP archive: {e}")))?;
    let scan = scan_archive(&mut archive, &opts.limits, false)?;
    let mut warnings = scan.warnings.clone();

    let mut members: Vec<(String, usize)> = scan.names.iter().map(|(k, v)| (k.clone(), *v)).collect();
    members.sort();
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    let mut files: Vec<(String, usize, u64)> = vec![];
    for (n, i) in members {
        if n.starts_with("__MACOSX/") {
            if !n.ends_with('/') {
                warnings.push(format!("skipped macOS metadata file {n:?}"));
            }
            continue;
        }
        let f = archive.by_index_raw(i)?;
        if f.is_dir() || n.ends_with('/') {
            dirs.insert(n.trim_end_matches('/').to_string());
        } else {
            files.push((n, i, f.size()));
        }
    }
    for (n, _, _) in &files {
        let mut cur = n.as_str();
        while let Some(idx) = cur.rfind('/') {
            cur = &cur[..idx];
            dirs.insert(cur.to_string());
        }
    }
    // A name used as both file and folder is a conflict we refuse rather than guess about.
    let file_keys: BTreeSet<String> = files.iter().map(|(n, _, _)| collision_key(n)).collect();
    if let Some(d) = dirs.iter().find(|d| file_keys.contains(&collision_key(d))) {
        return Err(ParcelError::InvalidManifest(format!("{d:?} is both a file and a folder")));
    }

    let mut base = Manifest::new(name, format!("Imported from {}", zip_path.file_name().and_then(|n| n.to_str()).unwrap_or("ZIP archive")));
    base.metadata.insert("imported_from".into(), "zip".into());
    let bytes_total: u64 = files.iter().map(|f| f.2).sum();
    let mut w = PackageWriter::create(dest, base, opts.clone(), (files.len() + dirs.len()) as u64, bytes_total, cancel, progress)?;
    for d in &dirs {
        w.add_virtual(Entry::new(EntryKind::Folder, uuid::Uuid::new_v4().to_string(), d.clone()))?;
    }
    for (n, i, size) in files {
        cancel.check()?;
        let f = archive.by_index(i)?;
        let mod_time = f.last_modified().and_then(|t| {
            chrono::NaiveDate::from_ymd_opt(t.year() as i32, t.month() as u32, t.day() as u32)
                .and_then(|d| d.and_hms_opt(t.hour() as u32, t.minute() as u32, t.second() as u32))
                .map(|dt| dt.and_utc())
        });
        let mut entry = Entry::new(EntryKind::File, uuid::Uuid::new_v4().to_string(), n);
        entry.modified_at = mod_time;
        // Bound the read: a lying central directory cannot make us write more than declared + 1.
        let mut limited = f.take(size + 1);
        w.add_stream(entry, &mut limited as &mut dyn Read, Some(size))?;
    }
    let build = w.finish("", None)?;
    Ok(ZipImportReport { build, warnings })
}
