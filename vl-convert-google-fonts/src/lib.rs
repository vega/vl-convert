//! Download and cache Google Fonts, resolve font variants, and track download usage.
//!
//! Use [`GoogleFontsClient`] for asynchronous or blocking requests. Enable the
//! `fontdb` feature to register downloaded fonts with a `fontdb::Database`.

mod cache;
mod client;
mod config;
mod error;
mod resolve;
mod types;
pub use client::GoogleFontsClient;
pub use config::{google_fonts_cache_dir, ClientConfig};
pub use error::{GoogleFontsError, GoogleFontsFailure, GoogleFontsResult};

#[cfg(feature = "fontdb")]
mod fontdb_ext;

#[cfg(feature = "fontdb")]
pub use fontdb_ext::{GoogleFontsDatabaseExt, RegisteredFontBatch};

pub use types::{
    family_to_id, find_closest_variant, FontLoadRequest, FontLoadResult, FontProbeResult,
    FontStyle, GoogleFontStats, GoogleFontUsage, LoadedFontBatch, UsedGoogleFontVariant,
    VariantRequest, VariantResolutionResult,
};
