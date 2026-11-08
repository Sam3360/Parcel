use parcel_core::ParcelError;
use parcel_workspace::WsError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct IpcError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

pub type IpcResult<T> = Result<T, IpcError>;

impl IpcError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self { code: "invalid".into(), message: msg.into(), details: None }
    }
}

impl From<WsError> for IpcError {
    fn from(e: WsError) -> Self {
        let details = match &e {
            WsError::SourceProblems { missing, changed } => Some(serde_json::json!({"missing": missing, "changed": changed})),
            _ => None,
        };
        Self { code: e.code().into(), message: e.to_string(), details }
    }
}
impl From<ParcelError> for IpcError {
    fn from(e: ParcelError) -> Self {
        Self { code: e.code().into(), message: e.to_string(), details: None }
    }
}
impl From<std::io::Error> for IpcError {
    fn from(e: std::io::Error) -> Self {
        Self { code: "io".into(), message: e.to_string(), details: None }
    }
}
impl From<serde_json::Error> for IpcError {
    fn from(e: serde_json::Error) -> Self {
        Self { code: "json".into(), message: e.to_string(), details: None }
    }
}
