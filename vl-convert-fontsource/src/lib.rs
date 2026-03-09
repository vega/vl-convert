mod cache;
mod client;
mod config;
pub mod error;
mod merge;
mod resolve;
pub mod types;
pub use client::FontsourceClient;
pub use config::{fontsource_cache_dir, ClientConfig};
pub use error::FontsourceError;

#[cfg(feature = "fontdb")]
mod fontdb_ext;

#[cfg(feature = "fontdb")]
pub use fontdb_ext::{FontsourceDatabaseExt, RegisteredFontBatch};

pub use types::{family_to_id, FontStyle, LoadedFontBatch, VariantRequest};
