//! The `.parcel` manifest: data model, serialization, validation and migration.
use crate::error::{ParcelError, Result};
use crate::limits::ResourceLimits;
use crate::pathsafe::{collision_key, validate_logical_path};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const FORMAT_ID: &str = "parcel";
/// Current format version written by this engine.
pub const FORMAT_VERSION: &str = "1.0";
pub const SUPPORTED_MAJOR: u32 = 1;
pub const MANIFEST_PATH: &str = "manifest.json";
pub const CONTENT_DIR: &str = "content/";
pub const NOTES_PATH: &str = "metadata/notes.md";
pub const REFERENCES_PATH: &str = "metadata/references.json";
pub const THUMBNAIL_PATH: &str = "previews/thumbnail.png";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Folder,
    Note,
    Link,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub kind: EntryKind,
    /// Logical path inside the package, forward slashes, no leading slash.
    pub path: String,
    /// Archive path of the stored bytes (files and notes only), always under `content/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_path: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<DateTime<Utc>>,
}

impl Entry {
    pub fn new(kind: EntryKind, id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            path: path.into(),
            storage_path: None,
            size: 0,
            sha256: None,
            mime: None,
            url: None,
            description: String::new(),
            tags: vec![],
            metadata: BTreeMap::new(),
            modified_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reference {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    /// Relies on another entry inside this package.
    Entry,
    /// Relies on something outside the package (declared only; never fetched).
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dependency {
    pub id: String,
    pub kind: DependencyKind,
    /// Entry id that declares the dependency.
    pub from_entry: String,
    /// Entry id (kind=entry) or free-form description/URL (kind=external).
    pub target: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetadataFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Generator {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub format: String,
    pub format_version: String,
    pub package_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    /// Incremented each time the same package lineage is re-exported.
    #[serde(default = "one")]
    pub revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_revision_id: Option<String>,
    pub generator: Generator,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub references: Vec<Reference>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub metadata_files: Vec<MetadataFile>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

fn one() -> u32 {
    1
}

impl Manifest {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            format: FORMAT_ID.into(),
            format_version: FORMAT_VERSION.into(),
            package_id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            created_at: now,
            modified_at: now,
            revision: 1,
            previous_revision_id: None,
            generator: Generator { name: "parcel-core".into(), version: env!("CARGO_PKG_VERSION").into() },
            entries: vec![],
            references: vec![],
            dependencies: vec![],
            metadata_files: vec![],
            metadata: BTreeMap::new(),
        }
    }

    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn total_content_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    pub fn entry_by_id(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

/// Parse and migrate raw manifest bytes into the current in-memory model, then validate.
pub fn parse_manifest(bytes: &[u8], limits: &ResourceLimits) -> Result<Manifest> {
    if bytes.len() as u64 > limits.max_manifest_bytes {
        return Err(ParcelError::LimitExceeded("manifest too large".into()));
    }
    let mut value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| ParcelError::InvalidManifest(format!("not valid JSON: {e}")))?;
    migrate(&mut value)?;
    let m: Manifest = serde_json::from_value(value).map_err(|e| ParcelError::InvalidManifest(format!("schema mismatch: {e}")))?;
    validate_manifest(&m, limits)?;
    Ok(m)
}

/// Upgrade older manifest JSON in place. Version 1.x needs no changes today; the hook exists
/// so future format majors can be handled without touching callers.
pub fn migrate(value: &mut serde_json::Value) -> Result<()> {
    let version = value
        .get("format_version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ParcelError::InvalidManifest("missing format_version".into()))?
        .to_string();
    let major = parse_major(&version)?;
    if major > SUPPORTED_MAJOR {
        return Err(ParcelError::UnsupportedVersion(version));
    }
    Ok(())
}

fn parse_major(version: &str) -> Result<u32> {
    version
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ParcelError::InvalidManifest(format!("bad format_version {version:?}")))
}

pub fn is_valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Reference/link URLs: only inert, well-known schemes. `javascript:`, `file:` and friends are refused.
pub fn is_allowed_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    !url.chars().any(|c| c.is_control())
        && url.len() <= 4096
        && (lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") || lower.starts_with("ftp://"))
}

pub fn validate_manifest(m: &Manifest, limits: &ResourceLimits) -> Result<()> {
    let inv = |s: String| Err(ParcelError::InvalidManifest(s));
    if m.format != FORMAT_ID {
        return inv(format!("format must be {FORMAT_ID:?}, found {:?}", m.format));
    }
    if parse_major(&m.format_version)? > SUPPORTED_MAJOR {
        return Err(ParcelError::UnsupportedVersion(m.format_version.clone()));
    }
    if uuid::Uuid::parse_str(&m.package_id).is_err() {
        return inv("package_id is not a UUID".into());
    }
    if m.name.trim().is_empty() || m.name.len() > 512 {
        return inv("package name must be 1-512 characters".into());
    }
    if m.entries.len() > limits.max_entries {
        return Err(ParcelError::LimitExceeded(format!("{} entries exceeds limit {}", m.entries.len(), limits.max_entries)));
    }
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    let mut storage = HashSet::new();
    let mut total: u64 = 0;
    for e in &m.entries {
        if e.id.is_empty() || e.id.len() > 128 || !ids.insert(e.id.clone()) {
            return inv(format!("entry id {:?} is empty, too long or duplicated", e.id));
        }
        validate_logical_path(&e.path, limits)?;
        if !paths.insert(collision_key(&e.path)) {
            return inv(format!("conflicting duplicate path {:?}", e.path));
        }
        total = total.saturating_add(e.size);
        if e.size > limits.max_entry_uncompressed {
            return Err(ParcelError::LimitExceeded(format!("entry {:?} too large", e.path)));
        }
        match e.kind {
            EntryKind::File | EntryKind::Note => {
                let sp = e
                    .storage_path
                    .as_deref()
                    .ok_or_else(|| ParcelError::InvalidManifest(format!("entry {:?} lacks storage_path", e.path)))?;
                validate_logical_path(sp, limits)?;
                if !sp.starts_with(CONTENT_DIR) {
                    return inv(format!("storage_path {sp:?} must be under content/"));
                }
                if !storage.insert(collision_key(sp)) {
                    return inv(format!("duplicate storage_path {sp:?}"));
                }
                match e.sha256.as_deref() {
                    Some(h) if is_valid_sha256(h) => {}
                    _ => return inv(format!("entry {:?} has missing/invalid sha256", e.path)),
                }
            }
            EntryKind::Folder => {
                if e.storage_path.is_some() || e.sha256.is_some() || e.size != 0 {
                    return inv(format!("folder {:?} must not carry content", e.path));
                }
            }
            EntryKind::Link => {
                let url = e.url.as_deref().unwrap_or("");
                if !is_allowed_url(url) {
                    return inv(format!("link {:?} has a missing or disallowed URL", e.path));
                }
            }
        }
        for (k, v) in &e.metadata {
            if k.is_empty() || k.len() > 256 || v.len() > 64 * 1024 {
                return inv(format!("invalid metadata key/value on {:?}", e.path));
            }
        }
    }
    if total > limits.max_total_uncompressed {
        return Err(ParcelError::LimitExceeded("declared total size exceeds limit".into()));
    }
    // A file may not also act as a directory for other entries.
    let kinds: BTreeMap<String, EntryKind> = m.entries.iter().map(|e| (collision_key(&e.path), e.kind)).collect();
    for e in &m.entries {
        let key = collision_key(&e.path);
        let mut cur = key.as_str();
        while let Some(idx) = cur.rfind('/') {
            cur = &cur[..idx];
            if let Some(k) = kinds.get(cur) {
                if *k != EntryKind::Folder {
                    return inv(format!("path {:?} is nested under non-folder entry", e.path));
                }
            }
        }
    }
    let mut rids = HashSet::new();
    for r in &m.references {
        if r.id.is_empty() || !rids.insert(r.id.clone()) {
            return inv("reference id empty or duplicated".into());
        }
        if !is_allowed_url(&r.url) {
            return inv(format!("reference {:?} has a disallowed URL scheme", r.title));
        }
        if let Some(eid) = &r.entry_id {
            if !ids.contains(eid) {
                return inv(format!("reference {:?} points at unknown entry", r.title));
            }
        }
    }
    let mut dids = HashSet::new();
    for d in &m.dependencies {
        if d.id.is_empty() || !dids.insert(d.id.clone()) {
            return inv("dependency id empty or duplicated".into());
        }
        if !ids.contains(&d.from_entry) {
            return inv(format!("dependency {:?} declared by unknown entry", d.id));
        }
    }
    for mf in &m.metadata_files {
        validate_logical_path(&mf.path, limits)?;
        if !is_valid_sha256(&mf.sha256) {
            return inv(format!("metadata file {:?} has invalid sha256", mf.path));
        }
    }
    Ok(())
}
