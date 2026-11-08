//! Package intelligence: duplicates, source status, dependency diagnostics.
use crate::error::Result;
use crate::hash::sha256_file;
use crate::manifest::{DependencyKind, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashedItem {
    pub id: String,
    pub label: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DuplicateGroup {
    pub sha256: String,
    pub size: u64,
    pub ids: Vec<String>,
    pub labels: Vec<String>,
    /// Bytes that would be saved by keeping a single copy.
    pub wasted_bytes: u64,
}

/// Group items with identical content. Empty files are ignored (they are trivially identical).
pub fn find_duplicates(items: &[HashedItem]) -> Vec<DuplicateGroup> {
    let mut by_hash: BTreeMap<(&str, u64), Vec<&HashedItem>> = BTreeMap::new();
    for it in items.iter().filter(|i| i.size > 0) {
        by_hash.entry((it.sha256.as_str(), it.size)).or_default().push(it);
    }
    let mut out: Vec<DuplicateGroup> = by_hash
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|((h, size), v)| DuplicateGroup {
            sha256: h.to_string(),
            size,
            ids: v.iter().map(|i| i.id.clone()).collect(),
            labels: v.iter().map(|i| i.label.clone()).collect(),
            wasted_bytes: size * (v.len() as u64 - 1),
        })
        .collect();
    out.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes).then(a.sha256.cmp(&b.sha256)));
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
    pub id: String,
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    /// Source modification time in Unix milliseconds at collection time.
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SourceStatus {
    Unchanged,
    Changed { reason: String },
    Missing,
    Unreadable { reason: String },
}

pub fn mtime_ms(path: &Path) -> Option<i64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = m.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_millis() as i64)
}

/// Compare a source file with what was recorded at collection time. Read-only.
/// `deep` forces a content hash even when size and timestamp match.
pub fn check_source(rec: &SourceRecord, deep: bool) -> SourceStatus {
    let meta = match std::fs::metadata(&rec.path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return SourceStatus::Missing,
        Err(e) => return SourceStatus::Unreadable { reason: e.to_string() },
    };
    if !meta.is_file() {
        return SourceStatus::Changed { reason: "source is no longer a regular file".into() };
    }
    if meta.len() != rec.size {
        return SourceStatus::Changed { reason: format!("size changed from {} to {} bytes", rec.size, meta.len()) };
    }
    let stamp_same = rec.modified_ms.is_some() && mtime_ms(&rec.path) == rec.modified_ms;
    if stamp_same && !deep {
        return SourceStatus::Unchanged;
    }
    match sha256_file(&rec.path) {
        Ok((h, _)) if h.eq_ignore_ascii_case(&rec.sha256) => SourceStatus::Unchanged,
        Ok(_) => SourceStatus::Changed { reason: "content differs from collected copy".into() },
        Err(e) => SourceStatus::Unreadable { reason: e.to_string() },
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplacementCheck {
    pub size: u64,
    pub sha256: String,
    /// True when the replacement has exactly the collected content.
    pub identical: bool,
}

/// Validate a user-chosen replacement for a missing/changed source.
pub fn validate_replacement(path: &Path, expected_sha256: &str) -> Result<ReplacementCheck> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(crate::error::ParcelError::Other("replacement is not a regular file".into()));
    }
    let (sha256, size) = sha256_file(path)?;
    Ok(ReplacementCheck { identical: sha256.eq_ignore_ascii_case(expected_sha256), size, sha256 })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub entry_id: Option<String>,
}

/// Static diagnostics over declared dependencies and references. Never touches the network.
pub fn diagnose_dependencies(m: &Manifest) -> Vec<Diagnostic> {
    let mut out = vec![];
    let ids: HashSet<&str> = m.entries.iter().map(|e| e.id.as_str()).collect();
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in &m.dependencies {
        match d.kind {
            DependencyKind::Entry => {
                if !ids.contains(d.target.as_str()) {
                    out.push(Diagnostic {
                        severity: if d.required { Severity::Error } else { Severity::Warning },
                        code: "dependency.missing_target".into(),
                        message: format!("dependency {:?} points at an entry that is not in the package", d.id),
                        entry_id: Some(d.from_entry.clone()),
                    });
                } else {
                    graph.entry(d.from_entry.as_str()).or_default().push(d.target.as_str());
                    if d.from_entry == d.target {
                        out.push(Diagnostic {
                            severity: Severity::Error,
                            code: "dependency.self".into(),
                            message: "an entry cannot depend on itself".into(),
                            entry_id: Some(d.from_entry.clone()),
                        });
                    }
                }
            }
            DependencyKind::External => out.push(Diagnostic {
                severity: Severity::Info,
                code: "dependency.external".into(),
                message: format!("declares external dependency: {}", d.target),
                entry_id: Some(d.from_entry.clone()),
            }),
        }
    }
    // Cycle detection (iterative DFS).
    let mut state: HashMap<&str, u8> = HashMap::new();
    for start in graph.keys().copied() {
        if state.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        state.insert(start, 1);
        while let Some((node, idx)) = stack.pop() {
            let next = graph.get(node).and_then(|v| v.get(idx)).copied();
            match next {
                None => {
                    state.insert(node, 2);
                }
                Some(n) => {
                    stack.push((node, idx + 1));
                    match state.get(n).copied().unwrap_or(0) {
                        0 => {
                            state.insert(n, 1);
                            stack.push((n, 0));
                        }
                        1 => out.push(Diagnostic {
                            severity: Severity::Warning,
                            code: "dependency.cycle".into(),
                            message: "circular dependency detected".into(),
                            entry_id: Some(n.to_string()),
                        }),
                        _ => {}
                    }
                }
            }
        }
    }
    for r in &m.references {
        if r.title.trim().is_empty() {
            out.push(Diagnostic {
                severity: Severity::Warning,
                code: "reference.untitled".into(),
                message: format!("reference to {} has no title", r.url),
                entry_id: r.entry_id.clone(),
            });
        }
    }
    out
}
