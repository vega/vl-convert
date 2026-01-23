// Allow uninlined format args for cleaner bail!/anyhow! macros
#![allow(clippy::uninlined_format_args)]
#![doc = include_str!("../README.md")]

pub mod bundler;
pub mod converter;
pub mod deno_stubs;
pub mod html;
pub mod image_loading;
pub mod module_loader;
pub mod text;

#[macro_use]
extern crate lazy_static;

// extern crate deno_core makes it available at crate root for op2 and extension! macros
extern crate deno_core;

pub use converter::VlConverter;
pub use deno_core::anyhow;
pub use module_loader::import_map::VlVersion;
pub use serde_json;

/// V8 snapshot containing the pre-compiled vl_convert_runtime extension.
/// This is generated at build time when the `snapshot` feature is enabled.
#[cfg(feature = "snapshot")]
pub static VL_CONVERT_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/VL_CONVERT_SNAPSHOT.bin"));
