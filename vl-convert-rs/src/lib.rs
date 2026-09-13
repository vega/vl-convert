#![doc = include_str!("../README.md")]

/// Logging macros that set `target: "vl_convert"` so that log messages are
/// attributed to `vl_convert` rather than the internal crate name `vl_convert_rs`.
macro_rules! vl_warn  { ($($arg:tt)*) => { log::warn!(target: "vl_convert", $($arg)*) }; }
macro_rules! vl_info  { ($($arg:tt)*) => { log::info!(target: "vl_convert", $($arg)*) }; }
macro_rules! vl_error { ($($arg:tt)*) => { log::error!(target: "vl_convert", $($arg)*) }; }
macro_rules! vl_debug { ($($arg:tt)*) => { log::debug!(target: "vl_convert", $($arg)*) }; }

/// Chart conversions, configuration, output options, and diagnostics.
pub mod converter;
pub(crate) mod data_ops;
mod deno_emit;
mod deno_stubs;
mod extract;
mod font_embed;
mod html;
mod image_loading;
mod module_loader;
pub(crate) mod svg_font;
mod text;

#[macro_use]
extern crate lazy_static;

// extern crate deno_core makes it available at crate root for op2 and extension! macros
extern crate deno_core;

#[doc(inline)]
pub use converter::{
    vega_to_url, vegalite_to_url, vlc_config_path, BaseUrlSetting, FormatLocale, GoogleFontRequest,
    GoogleFontStats, GoogleFontUsage, HtmlOpts, HtmlOutput, JpegOpts, JpegOutput, LogEntry,
    LogLevel, MissingFontsPolicy, PdfOpts, PdfOutput, PngOpts, PngOutput, Renderer,
    ScenegraphMsgpackOutput, ScenegraphOutput, SvgOpts, SvgOutput, TimeFormatLocale, UrlOpts,
    UsedGoogleFontVariant, ValueOrString, VegaOutput, VgOpts, VlConverter, VlOpts, VlcConfig,
    WorkerMemoryUsage,
};
pub use deno_core::anyhow;
#[doc(inline)]
pub use extract::{FontInfo, FontSource, FontVariant};
#[doc(inline)]
pub use module_loader::import_map::{
    VlVersion, VEGA_EMBED_VERSION, VEGA_THEMES_VERSION, VEGA_VERSION, VL_VERSIONS,
};
pub use module_loader::{FORMATE_LOCALE_MAP, TIME_FORMATE_LOCALE_MAP};
pub use serde_json;

pub use module_loader::import_map::DEFAULT_VL_VERSION;
#[doc(inline)]
pub use text::{
    current_font_directories, current_google_fonts_cache_size_mb, register_font_directory,
    set_font_directories, set_google_fonts_cache_size_mb, DEFAULT_GOOGLE_FONTS_CACHE_SIZE_MB,
};
pub use vl_convert_google_fonts::{google_fonts_cache_dir, FontStyle, VariantRequest};

/// V8 snapshot containing the pre-compiled deno_runtime extensions plus the
/// vl_convert_runtime extension. Generated at build time for container
/// compatibility and faster startup.
pub(crate) static VL_CONVERT_SNAPSHOT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/VL_CONVERT_SNAPSHOT.bin"));

pub(crate) use converter::inner::with_font_overlay;

include!(concat!(env!("OUT_DIR"), "/VL_CONVERT_RESIDUAL_SOURCES.rs"));
