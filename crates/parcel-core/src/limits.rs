//! Configurable resource limits applied to untrusted archives.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ResourceLimits {
    pub max_entries: usize,
    pub max_total_uncompressed: u64,
    pub max_entry_uncompressed: u64,
    /// Maximum allowed uncompressed/compressed ratio for a single entry (ZIP bomb guard).
    pub max_compression_ratio: u64,
    pub max_manifest_bytes: u64,
    pub max_path_len: usize,
    pub max_path_depth: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_total_uncompressed: 64 * 1024 * 1024 * 1024,
            max_entry_uncompressed: 32 * 1024 * 1024 * 1024,
            max_compression_ratio: 1000,
            max_manifest_bytes: 64 * 1024 * 1024,
            max_path_len: 1024,
            max_path_depth: 64,
        }
    }
}
