#![doc = include_str!("../README.md")]

/// Logging macros that set `target: "vl_convert"` so that log messages are
/// attributed to `vl_convert` rather than the internal crate name `vl_convert_rs`.
macro_rules! vl_warn  { ($($arg:tt)*) => { log::warn!(target: "vl_convert", $($arg)*) }; }
macro_rules! vl_info  { ($($arg:tt)*) => { log::info!(target: "vl_convert", $($arg)*) }; }
macro_rules! vl_error { ($($arg:tt)*) => { log::error!(target: "vl_convert", $($arg)*) }; }
macro_rules! vl_debug { ($($arg:tt)*) => { log::debug!(target: "vl_convert", $($arg)*) }; }

pub mod converter;
pub(crate) mod data_ops;
pub mod deno_emit;
pub mod deno_stubs;
pub mod extract;
pub mod font_embed;
pub mod html;
pub mod image_loading;
pub mod module_loader;
pub(crate) mod svg_font;
pub mod text;

#[macro_use]
extern crate lazy_static;

// extern crate deno_core makes it available at crate root for op2 and extension! macros
extern crate deno_core;

#[allow(deprecated)]
pub use converter::{
    vlc_config_path, BaseUrlSetting, GoogleFontRequest, HtmlOpts, JpegOpts, PdfOpts, PngOpts,
    Renderer, SvgOpts, VgOpts, VlcConfig, VlConverter, VlConverterConfig, VlOpts, WorkerMemoryUsage,
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
