//! Package comparison. Content equality is decided purely by SHA-256 checksum; no semantic diffing.
use crate::manifest::{Entry, Manifest};
use crate::pathsafe::collision_key;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EntryDiff {
    pub path: String,
    pub kind: String,
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Comparison {
    pub added: Vec<EntryDiff>,
    pub removed: Vec<EntryDiff>,
    /// Same path, different content checksum.
    pub changed: Vec<EntryDiff>,
    /// Same content checksum at a different path (reported instead of add+remove).
    pub moved: Vec<EntryDiff>,
    /// Same path and content, but description/tags/metadata/url differ.
    pub metadata_changed: Vec<EntryDiff>,
    pub unchanged: Vec<String>,
    pub package_differences: Vec<String>,
}

impl Comparison {
    pub fn is_identical(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.moved.is_empty()
            && self.metadata_changed.is_empty()
            && self.package_differences.is_empty()
    }
}

fn kind_str(e: &Entry) -> String {
    format!("{:?}", e.kind).to_lowercase()
}

fn meta_diffs(a: &Entry, b: &Entry) -> Vec<String> {
    let mut d = vec![];
    if a.description != b.description {
        d.push("description changed".to_string());
    }
    if a.tags != b.tags {
        d.push(format!("tags changed: {:?} → {:?}", a.tags, b.tags));
    }
    if a.url != b.url {
        d.push("url changed".to_string());
    }
    for k in a.metadata.keys().chain(b.metadata.keys()).collect::<std::collections::BTreeSet<_>>() {
        match (a.metadata.get(k), b.metadata.get(k)) {
            (Some(x), Some(y)) if x != y => d.push(format!("metadata {k:?}: {x:?} → {y:?}")),
            (Some(_), None) => d.push(format!("metadata {k:?} removed")),
            (None, Some(_)) => d.push(format!("metadata {k:?} added")),
            _ => {}
        }
    }
    d
}

pub fn compare_manifests(old: &Manifest, new: &Manifest) -> Comparison {
    let mut c = Comparison::default();
    if old.name != new.name {
        c.package_differences.push(format!("name: {:?} → {:?}", old.name, new.name));
    }
    if old.description != new.description {
        c.package_differences.push("description changed".into());
    }
    for k in old.metadata.keys().chain(new.metadata.keys()).collect::<std::collections::BTreeSet<_>>() {
        if old.metadata.get(k) != new.metadata.get(k) {
            c.package_differences.push(format!("package metadata {k:?} differs"));
        }
    }
    if old.references.len() != new.references.len() {
        c.package_differences.push(format!("reference count: {} → {}", old.references.len(), new.references.len()));
    }
    if old.dependencies.len() != new.dependencies.len() {
        c.package_differences.push(format!("dependency count: {} → {}", old.dependencies.len(), new.dependencies.len()));
    }
    let old_map: BTreeMap<String, &Entry> = old.entries.iter().map(|e| (collision_key(&e.path), e)).collect();
    let new_map: BTreeMap<String, &Entry> = new.entries.iter().map(|e| (collision_key(&e.path), e)).collect();

    let mut removed: Vec<&Entry> = vec![];
    let mut added: Vec<&Entry> = vec![];
    for (k, a) in &old_map {
        match new_map.get(k) {
            None => removed.push(a),
            Some(b) => {
                if a.kind != b.kind {
                    c.changed.push(EntryDiff {
                        path: b.path.clone(),
                        kind: kind_str(b),
                        detail: vec![format!("type {} → {}", kind_str(a), kind_str(b))],
                    });
                } else if a.sha256 != b.sha256 || a.size != b.size {
                    c.changed.push(EntryDiff {
                        path: b.path.clone(),
                        kind: kind_str(b),
                        detail: vec![format!("size {} → {} bytes", a.size, b.size), "content checksum differs".into()],
                    });
                } else {
                    let md = meta_diffs(a, b);
                    if md.is_empty() {
                        c.unchanged.push(b.path.clone());
                    } else {
                        c.metadata_changed.push(EntryDiff { path: b.path.clone(), kind: kind_str(b), detail: md });
                    }
                }
            }
        }
    }
    for (k, b) in &new_map {
        if !old_map.contains_key(k) {
            added.push(b);
        }
    }
    // Pair removed/added files with identical content as moves.
    let mut removed_by_hash: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, e) in removed.iter().enumerate() {
        if let (Some(h), true) = (e.sha256.as_deref(), e.size > 0) {
            removed_by_hash.entry(h).or_default().push(i);
        }
    }
    let mut consumed = vec![false; removed.len()];
    let mut still_added = vec![];
    for b in added {
        let hit = b.sha256.as_deref().and_then(|h| removed_by_hash.get_mut(h)).and_then(|v| v.pop());
        match hit {
            Some(i) => {
                consumed[i] = true;
                c.moved.push(EntryDiff {
                    path: b.path.clone(),
                    kind: kind_str(b),
                    detail: vec![format!("moved from {}", removed[i].path)],
                });
            }
            None => still_added.push(b),
        }
    }
    c.added = still_added.iter().map(|e| EntryDiff { path: e.path.clone(), kind: kind_str(e), detail: vec![] }).collect();
    c.removed = removed
        .iter()
        .enumerate()
        .filter(|(i, _)| !consumed[*i])
        .map(|(_, e)| EntryDiff { path: e.path.clone(), kind: kind_str(e), detail: vec![] })
        .collect();
    c.unchanged.sort();
    c
}
