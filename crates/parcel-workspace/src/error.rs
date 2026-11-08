use serde::Serialize;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, WsError>;

#[derive(Debug, Clone, Serialize)]
pub struct SourceProblem {
    pub entry_id: String,
    pub path: String,
    pub source_path: String,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum WsError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Core(#[from] parcel_core::ParcelError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("a sibling named {0:?} already exists here")]
    NameConflict(String),
    #[error("{} source file(s) are missing or have changed since they were collected", .missing.len() + .changed.len())]
    SourceProblems { missing: Vec<SourceProblem>, changed: Vec<SourceProblem> },
    #[error("database is damaged: {0}")]
    Corrupt(String),
}

impl WsError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Db(_) => "database",
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::Core(e) => e.code(),
            Self::NotFound(_) => "not_found",
            Self::Invalid(_) => "invalid",
            Self::NameConflict(_) => "name_conflict",
            Self::SourceProblems { .. } => "source_problems",
            Self::Corrupt(_) => "corrupt_database",
        }
    }
}
