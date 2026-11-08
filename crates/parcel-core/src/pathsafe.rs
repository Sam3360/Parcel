//! Validation of logical (in-archive) paths. Archive paths are always untrusted.
use crate::error::{ParcelError, Result};
use crate::limits::ResourceLimits;
use std::path::{Component, Path, PathBuf};

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4",
    "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate a forward-slash separated relative path coming from an archive or manifest.
pub fn validate_logical_path(path: &str, limits: &ResourceLimits) -> Result<()> {
    let bad = |why: &str| Err(ParcelError::UnsafePath(format!("{path:?}: {why}")));
    if path.is_empty() {
        return bad("empty path");
    }
    if path.len() > limits.max_path_len {
        return bad("path too long");
    }
    if path.contains('\0') || path.chars().any(|c| c.is_control()) {
        return bad("control characters");
    }
    if path.contains('\\') {
        return bad("backslash separators are not allowed");
    }
    if path.starts_with('/') || path.starts_with("//") {
        return bad("absolute path");
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return bad("drive-letter path");
    }
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() > limits.max_path_depth {
        return bad("path too deep");
    }
    for part in parts {
        if part.is_empty() {
            return bad("empty path component");
        }
        if part == "." || part == ".." {
            return bad("relative traversal component");
        }
        if part.ends_with('.') || part.ends_with(' ') {
            return bad("component ends with dot or space");
        }
        if part.contains([':', '*', '?', '"', '<', '>', '|']) {
            return bad("characters not allowed in Windows file names");
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if WINDOWS_RESERVED.contains(&stem.as_str()) {
            return bad("reserved Windows device name");
        }
    }
    Ok(())
}

/// Case-insensitive key used to detect conflicting duplicate paths (Windows is case-insensitive).
pub fn collision_key(path: &str) -> String {
    path.trim_end_matches('/').to_lowercase()
}

/// Turn an arbitrary display name into a safe single path component.
pub fn sanitize_component(name: &str) -> String {
    let mut out: String = name.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    out = out.trim_matches(|c| c == '.' || c == ' ').to_string();
    if out.is_empty() {
        out = "untitled".into();
    }
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        out.insert(0, '_');
    }
    out
}

/// Join a validated logical path onto a destination directory, guaranteeing containment.
pub fn safe_join(dest: &Path, logical: &str, limits: &ResourceLimits) -> Result<PathBuf> {
    validate_logical_path(logical, limits)?;
    let mut out = dest.to_path_buf();
    for part in logical.trim_end_matches('/').split('/') {
        out.push(part);
    }
    // Defence in depth: ensure no component escapes.
    let rel = out.strip_prefix(dest).map_err(|_| ParcelError::UnsafePath(logical.into()))?;
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(ParcelError::UnsafePath(logical.into()));
    }
    Ok(out)
}
