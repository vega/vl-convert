mod cache;
mod client;
mod config;
pub mod error;
mod ext;
mod resolve;
pub mod types;

pub use client::FontsourceClient;
pub use config::ClientConfig;
pub use error::FontsourceFontdbError;
pub use ext::FontsourceDatabaseExt;
pub use types::{
    family_to_id, FontStyle, LoadedFontBatch, RegisteredFontBatch, VariantRequest,
};
