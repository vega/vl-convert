mod cache;
mod client;
mod config;
pub mod error;
mod resolve;
pub mod types;
pub use client::GoogleFontsClient;
pub use config::{google_fonts_cache_dir, ClientConfig};
pub use error::GoogleFontsError;

#[cfg(feature = "fontdb")]
mod fontdb_ext;

#[cfg(feature = "fontdb")]
pub use fontdb_ext::{GoogleFontsDatabaseExt, RegisteredFontBatch};

pub use types::{family_to_id, FontStyle, LoadedFontBatch, VariantRequest};
