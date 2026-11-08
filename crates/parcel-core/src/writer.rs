//! Streaming, atomic package writer.
use crate::cancel::{CancelToken, Progress, ProgressFn};
use crate::error::{ParcelError, Result};
use crate::hash::BUF;
use crate::limits::ResourceLimits;
use crate::manifest::*;
use crate::mime;
use crate::pathsafe::{collision_key, validate_logical_path};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub overwrite: bool,
    pub compress: bool,
    pub verify_after_write: bool,
    pub limits: ResourceLimits,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { overwrite: false, compress: true, verify_after_write: true, limits: ResourceLimits::default() }
    }
}

pub enum ItemContent {
    None,
    File(PathBuf),
    Bytes(Vec<u8>),
}

pub struct BuildItem {
    pub entry: Entry,
    pub content: ItemContent,
}

/// Everything needed to produce a package, independent of any database or UI.
pub struct BuildInput {
    pub base: Manifest,
    pub items: Vec<BuildItem>,
    pub workspace_notes: String,
    pub thumbnail_png: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct BuildReport {
    pub path: PathBuf,
    pub package_size: u64,
    pub content_size: u64,
    pub entry_count: usize,
    pub manifest: Manifest,
}

pub struct PackageWriter<'a> {
    zip: Option<ZipWriter<File>>,
    tmp: PathBuf,
    dest: PathBuf,
    opts: ExportOptions,
    manifest: Manifest,
    storage: HashSet<String>,
    paths: HashSet<String>,
    cancel: &'a CancelToken,
    progress: ProgressFn<'a>,
    items_total: u64,
    bytes_total: u64,
    items_done: u64,
    bytes_done: u64,
    finished: bool,
}

impl<'a> PackageWriter<'a> {
    pub fn create(
        dest: &Path,
        base: Manifest,
        opts: ExportOptions,
        items_total: u64,
        bytes_total: u64,
        cancel: &'a CancelToken,
        progress: ProgressFn<'a>,
    ) -> Result<Self> {
        if dest.exists() && !opts.overwrite {
            return Err(ParcelError::AlreadyExists(dest.display().to_string()));
        }
        let dir = dest.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("package.parcel");
        let tmp = dir.join(format!(".{name}.{}.parcel-tmp", uuid::Uuid::new_v4().simple()));
        let file = File::options().write(true).create_new(true).open(&tmp)?;
        let mut manifest = base;
        manifest.entries.clear();
        manifest.metadata_files.clear();
        Ok(Self {
            zip: Some(ZipWriter::new(file)),
            tmp,
            dest: dest.to_path_buf(),
            opts,
            manifest,
            storage: HashSet::new(),
            paths: HashSet::new(),
            cancel,
            progress,
            items_total,
            bytes_total,
            items_done: 0,
            bytes_done: 0,
            finished: false,
        })
    }

    fn report(&self, phase: &str, item: &str) {
        (self.progress)(&Progress {
            phase: phase.into(),
            current_item: item.into(),
            items_done: self.items_done,
            items_total: self.items_total,
            bytes_done: self.bytes_done,
            bytes_total: self.bytes_total,
        });
    }

    fn claim_path(&mut self, path: &str) -> Result<()> {
        validate_logical_path(path, &self.opts.limits)?;
        if !self.paths.insert(collision_key(path)) {
            return Err(ParcelError::InvalidManifest(format!("duplicate path {path:?} in package")));
        }
        if self.manifest.entries.len() >= self.opts.limits.max_entries {
            return Err(ParcelError::LimitExceeded("too many entries".into()));
        }
        Ok(())
    }

    /// Add a folder or link entry (no stored bytes).
    pub fn add_virtual(&mut self, entry: Entry) -> Result<()> {
        self.cancel.check()?;
        if !matches!(entry.kind, EntryKind::Folder | EntryKind::Link) {
            return Err(ParcelError::Other("add_virtual only accepts folders and links".into()));
        }
        self.claim_path(&entry.path)?;
        self.manifest.entries.push(entry);
        Ok(())
    }

    /// Stream `reader` into the archive as a file or note entry, computing size and SHA-256.
    /// `expected_size`, when known, is enforced: a source that changes length mid-read is rejected.
    pub fn add_stream(&mut self, mut entry: Entry, reader: &mut dyn Read, expected_size: Option<u64>) -> Result<()> {
        self.cancel.check()?;
        if !matches!(entry.kind, EntryKind::File | EntryKind::Note) {
            return Err(ParcelError::Other("add_stream only accepts files and notes".into()));
        }
        self.claim_path(&entry.path)?;
        let storage_path = format!("{CONTENT_DIR}{}", entry.path);
        validate_logical_path(&storage_path, &self.opts.limits)?;
        if !self.storage.insert(collision_key(&storage_path)) {
            return Err(ParcelError::InvalidManifest(format!("duplicate storage path {storage_path:?}")));
        }
        self.report("writing", &entry.path);

        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; BUF];
        // Peek the first chunk for MIME sniffing and compression choice.
        let first = read_full(reader, &mut buf)?;
        let mime_guess = mime::guess(&entry.path, &buf[..first.min(512)]);
        let method = if self.opts.compress && !mime::is_precompressed(mime_guess.as_deref()) {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        let big = expected_size.is_some_and(|s| s >= 3 * 1024 * 1024 * 1024);
        let options = SimpleFileOptions::default().compression_method(method).large_file(big);
        let zip = self.zip.as_mut().ok_or_else(|| ParcelError::Other("writer already finished".into()))?;
        zip.start_file(&storage_path, options)?;

        let mut total = 0u64;
        let mut n = first;
        loop {
            if n == 0 {
                break;
            }
            self.cancel.check()?;
            total += n as u64;
            if total > self.opts.limits.max_entry_uncompressed {
                return Err(ParcelError::LimitExceeded(format!("{:?} exceeds per-entry limit", entry.path)));
            }
            if self.bytes_done + n as u64 > self.opts.limits.max_total_uncompressed {
                return Err(ParcelError::LimitExceeded("total size exceeds limit".into()));
            }
            hasher.update(&buf[..n]);
            zip.write_all(&buf[..n])?;
            self.bytes_done += n as u64;
            if self.bytes_done % (BUF as u64 * 4) < n as u64 {
                (self.progress)(&Progress {
                    phase: "writing".into(),
                    current_item: entry.path.clone(),
                    items_done: self.items_done,
                    items_total: self.items_total,
                    bytes_done: self.bytes_done,
                    bytes_total: self.bytes_total,
                });
            }
            n = reader.read(&mut buf)?;
        }
        if let Some(exp) = expected_size {
            if exp != total {
                return Err(ParcelError::Other(format!(
                    "source for {:?} changed while exporting (expected {exp} bytes, read {total})",
                    entry.path
                )));
            }
        }
        entry.storage_path = Some(storage_path);
        entry.size = total;
        entry.sha256 = Some(hex::encode(hasher.finalize()));
        entry.mime = mime_guess;
        self.items_done += 1;
        self.manifest.entries.push(entry);
        Ok(())
    }

    pub fn add_bytes(&mut self, entry: Entry, data: &[u8]) -> Result<()> {
        let mut cur = std::io::Cursor::new(data);
        self.add_stream(entry, &mut cur, Some(data.len() as u64))
    }

    fn write_meta(&mut self, path: &str, data: &[u8]) -> Result<()> {
        let zip = self.zip.as_mut().ok_or_else(|| ParcelError::Other("writer already finished".into()))?;
        zip.start_file(path, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
        zip.write_all(data)?;
        self.manifest.metadata_files.push(MetadataFile {
            path: path.into(),
            size: data.len() as u64,
            sha256: crate::hash::sha256_bytes(data),
        });
        Ok(())
    }

    /// Write metadata files and the manifest, then atomically move the package into place.
    pub fn finish(mut self, notes: &str, thumbnail: Option<&[u8]>) -> Result<BuildReport> {
        self.cancel.check()?;
        self.report("finalizing", "metadata");
        self.write_meta(NOTES_PATH, notes.as_bytes())?;
        let refs = serde_json::to_vec_pretty(&self.manifest.references)?;
        self.write_meta(REFERENCES_PATH, &refs)?;
        if let Some(t) = thumbnail {
            self.write_meta(THUMBNAIL_PATH, t)?;
        }
        self.manifest.modified_at = chrono::Utc::now();
        validate_manifest(&self.manifest, &self.opts.limits)?;
        let json = self.manifest.to_json()?;
        {
            let zip = self.zip.as_mut().unwrap();
            zip.start_file(MANIFEST_PATH, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
            zip.write_all(&json)?;
        }
        let file = self.zip.take().unwrap().finish()?;
        file.sync_all()?;
        drop(file);
        self.cancel.check()?;

        if self.opts.verify_after_write {
            self.report("verifying", "written package");
            let reader = crate::reader::PackageReader::open(&self.tmp, &self.opts.limits)?;
            let rep = reader.verify(self.cancel, &|_p: &Progress| {})?;
            if !rep.ok {
                return Err(ParcelError::Other("post-write verification failed; package discarded".into()));
            }
        }
        if self.dest.exists() && !self.opts.overwrite {
            return Err(ParcelError::AlreadyExists(self.dest.display().to_string()));
        }
        fs::rename(&self.tmp, &self.dest)?;
        self.finished = true;
        let package_size = fs::metadata(&self.dest)?.len();
        Ok(BuildReport {
            path: self.dest.clone(),
            package_size,
            content_size: self.manifest.total_content_size(),
            entry_count: self.manifest.entries.len(),
            manifest: self.manifest.clone(),
        })
    }
}

impl Drop for PackageWriter<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.zip.take();
            let _ = fs::remove_file(&self.tmp);
        }
    }
}

fn read_full(r: &mut dyn Read, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = r.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// Build a package from an in-memory description plus source files on disk.
/// Source files are opened read-only and never modified.
pub fn build_package(
    input: BuildInput,
    dest: &Path,
    opts: ExportOptions,
    cancel: &CancelToken,
    progress: ProgressFn<'_>,
) -> Result<BuildReport> {
    let mut bytes_total = 0u64;
    for it in &input.items {
        match &it.content {
            ItemContent::File(p) => bytes_total += fs::metadata(p)?.len(),
            ItemContent::Bytes(b) => bytes_total += b.len() as u64,
            ItemContent::None => {}
        }
    }
    let mut w = PackageWriter::create(dest, input.base, opts, input.items.len() as u64, bytes_total, cancel, progress)?;
    for it in input.items {
        let mut entry = it.entry;
        match it.content {
            ItemContent::None => w.add_virtual(entry)?,
            ItemContent::Bytes(b) => w.add_bytes(entry, &b)?,
            ItemContent::File(p) => {
                let meta = fs::metadata(&p)?;
                if !meta.is_file() {
                    return Err(ParcelError::Other(format!("source is not a regular file: {}", p.display())));
                }
                if entry.modified_at.is_none() {
                    entry.modified_at = meta.modified().ok().map(Into::into);
                }
                let mut f = File::open(&p)?;
                w.add_stream(entry, &mut f, Some(meta.len()))?;
            }
        }
    }
    w.finish(&input.workspace_notes, input.thumbnail_png.as_deref())
}
