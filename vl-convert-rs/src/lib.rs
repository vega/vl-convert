#![doc = include_str!("../README.md")]

pub mod converter;
pub(crate) mod data_ops;
pub mod deno_emit;
pub mod deno_stubs;
pub mod extract;
pub mod font_embed;
pub mod html;
pub mod image_loading;
pub mod module_loader;
pub mod text;

#[macro_use]
extern crate lazy_static;

// extern crate deno_core makes it available at crate root for op2 and extension! macros
extern crate deno_core;

pub use converter::{
    BaseUrlSetting, GoogleFontRequest, Renderer, VgOpts, VlConverter, VlConverterConfig, VlOpts,
    WorkerMemoryUsage,
};
pub use deno_core::anyhow;
pub use extract::{FontInfo, FontSource, FontVariant};
pub use module_loader::import_map::VlVersion;
pub use serde_json;
pub use text::configure_font_cache;
pub use vl_convert_google_fonts::{google_fonts_cache_dir, FontStyle, VariantRequest};

/// V8 snapshot containing the pre-compiled deno_runtime extensions plus our
/// vl_convert_runtime extension. Generated at build time for container
/// compatibility and faster startup.
pub static VL_CONVERT_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/VL_CONVERT_SNAPSHOT.bin"));
