use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: String,
    pub notes: String,
    pub metadata: BTreeMap<String, String>,
    pub package_id: String,
    pub revision: i64,
    pub export_profile_id: Option<String>,
    pub template_id: Option<String>,
    pub last_export_path: Option<String>,
    pub created_at: String,
    pub modified_at: String,
    pub opened_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub entry_count: i64,
    pub total_size: i64,
    pub revision: i64,
    pub modified_at: String,
    pub opened_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkspaceUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<BTreeMap<String, String>>,
    pub export_profile_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub url: Option<String>,
    pub content: Option<String>,
    pub attached_to: Option<String>,
    pub sort_order: i64,
    /// "source" = file stays where it is; "imported" = extracted into Parcel's managed store.
    pub origin: String,
    pub source_path: Option<String>,
    pub size: i64,
    pub sha256: Option<String>,
    pub mime: Option<String>,
    pub source_mtime_ms: Option<i64>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub created_at: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EntryUpdate {
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<BTreeMap<String, String>>,
    pub content: Option<String>,
    pub url: Option<String>,
    pub attached_to: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefRow {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub category: String,
    pub entry_id: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefInput {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub entry_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepRow {
    pub id: String,
    pub workspace_id: String,
    pub from_entry: String,
    pub kind: String,
    pub target: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityRow {
    pub id: i64,
    pub ts: String,
    pub workspace_id: Option<String>,
    pub kind: String,
    pub message: String,
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentPackage {
    pub path: String,
    pub name: String,
    pub opened_at: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevisionRow {
    pub revision: i64,
    pub package_id: String,
    pub path: String,
    pub exported_at: String,
    pub entry_count: i64,
    pub package_size: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WorkspaceStats {
    pub files: i64,
    pub folders: i64,
    pub notes: i64,
    pub links: i64,
    pub references: i64,
    pub total_size: i64,
    pub tagged_entries: i64,
    pub largest: Vec<(String, i64)>,
    pub by_extension: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub text: String,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub ext: Option<String>,
    pub parent_id: Option<String>,
    /// name | size | modified | kind
    pub sort: Option<String>,
    #[serde(default)]
    pub descending: bool,
    pub limit: Option<i64>,
}
