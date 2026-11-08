//! parcel-core: the Parcel package engine. No dependency on Tauri or any UI.
pub mod cancel;
pub mod error;
pub mod hash;
pub mod limits;
pub mod manifest;
pub mod pathsafe;

pub use error::{ParcelError, Result};
pub mod compare;
pub mod intel;
pub mod mime;
pub mod reader;
pub mod writer;
pub mod zipimport;
