//! parcel-workspace: persistence and orchestration on top of parcel-core. No UI/Tauri dependency.
pub mod db;
pub mod error;
pub mod import;
pub mod launch;
pub mod models;
pub mod store;
pub mod templates;

pub use error::{Result, WsError};
pub use store::Store;
pub mod collect;
pub mod ops;
