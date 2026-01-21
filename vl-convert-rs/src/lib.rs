// Allow uninlined format args for cleaner bail!/anyhow! macros
#![allow(clippy::uninlined_format_args)]
#![doc = include_str!("../README.md")]

pub mod converter;
pub mod html;
pub mod image_loading;
pub mod module_loader;
pub mod text;

#[macro_use]
extern crate lazy_static;

pub use converter::VlConverter;
pub use deno_core::anyhow;
pub use module_loader::import_map::VlVersion;
pub use serde_json;
