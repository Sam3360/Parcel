//! Error type shared by every part of the package engine.
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ParcelError>;

#[derive(Debug, Error)]
pub enum ParcelError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("unsafe path rejected: {0}")]
    UnsafePath(String),
    #[error("resource limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("not a valid Parcel package: {0}")]
    NotAPackage(String),
    #[error("unsupported format version {0}")]
    UnsupportedVersion(String),
    #[error("destination already exists: {0}")]
    AlreadyExists(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl ParcelError {
    /// Stable machine-readable code for the UI layer.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Zip(_) => "archive",
            Self::Json(_) => "json",
            Self::InvalidManifest(_) => "invalid_manifest",
            Self::UnsafePath(_) => "unsafe_path",
            Self::LimitExceeded(_) => "limit_exceeded",
            Self::NotAPackage(_) => "not_a_package",
            Self::UnsupportedVersion(_) => "unsupported_version",
            Self::AlreadyExists(_) => "already_exists",
            Self::Cancelled => "cancelled",
            Self::Other(_) => "other",
        }
    }
}
