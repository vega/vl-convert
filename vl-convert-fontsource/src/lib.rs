mod cache;
mod client;
mod config;
pub mod error;
mod resolve;
pub mod types;

#[cfg(feature = "fontdb")]
mod fontdb_ext;

pub use client::FontsourceClient;
pub use config::ClientConfig;
pub use error::FontsourceError;

#[cfg(feature = "fontdb")]
pub use fontdb_ext::{FontsourceDatabaseExt, RegisteredFontBatch};

pub use types::{family_to_id, FontStyle, LoadedFontBatch, VariantRequest};
