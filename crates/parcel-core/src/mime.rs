//! Small, dependency-free MIME detection (extension + magic bytes). Never executes or renders content.
use std::path::Path;

pub fn sniff(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else if head.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else if head.starts_with(b"PK\x03\x04") {
        Some("application/zip")
    } else {
        None
    }
}

pub fn from_extension(ext: &str) -> Option<&'static str> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "html" | "htm" => "text/html",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" | "tsx" | "jsx" | "rs" | "py" | "c" | "h" | "cpp" | "java" | "go" | "sh" | "ps1" => "text/plain",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" | "parcel" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "exe" | "dll" | "msi" => "application/octet-stream",
        _ => return None,
    })
}

pub fn guess(path: &str, head: &[u8]) -> Option<String> {
    if let Some(m) = sniff(head) {
        // Zip-based office formats keep their extension-derived type.
        if m == "application/zip" {
            if let Some(e) = Path::new(path).extension().and_then(|e| e.to_str()) {
                if let Some(x) = from_extension(e) {
                    return Some(x.to_string());
                }
            }
        }
        return Some(m.to_string());
    }
    Path::new(path).extension().and_then(|e| e.to_str()).and_then(from_extension).map(String::from)
}

/// Formats that are already compressed; storing them avoids wasted CPU.
pub fn is_precompressed(mime: Option<&str>) -> bool {
    matches!(
        mime,
        Some("image/png" | "image/jpeg" | "image/gif" | "image/webp" | "application/zip" | "video/mp4" | "video/quicktime" | "audio/mpeg")
    ) || mime.is_some_and(|m| m.contains("openxmlformats"))
}
