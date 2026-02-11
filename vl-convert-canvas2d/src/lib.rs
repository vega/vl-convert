// Allow uninlined format args for cleaner bail!/anyhow! macros
#![allow(clippy::uninlined_format_args)]

//! Pure Rust Canvas 2D API implementation using tiny-skia and cosmic-text.
//!
//! This crate provides a Canvas 2D API implementation that can be used without
//! a browser or JavaScript runtime. It uses:
//! - `tiny-skia` for 2D graphics rendering
//! - `cosmic-text` for text shaping, measurement, and rendering
//! - `fontdb` for font database management (can be shared with other crates)
//!
//! # Example
//!
//! ```rust,ignore
//! use vl_convert_canvas2d::{Canvas2dContext, RectParams};
//!
//! let mut ctx = Canvas2dContext::new(400, 300)?;
//! ctx.set_fill_style("#ff0000");
//! ctx.fill_rect(&RectParams { x: 10.0, y: 10.0, width: 100.0, height: 50.0 });
//! let png_data = ctx.to_png()?;
//! ```

mod arc;
mod context;
mod error;
pub mod font_config;
mod font_parser;
mod geometry;
mod gradient;
mod path2d;
mod pattern;
mod style;
mod text;

// Re-export public API
pub use context::{Canvas2dContext, DOMMatrix};
pub use error::{Canvas2dError, Canvas2dResult};
pub use font_config::{
    font_config_to_fontdb, CustomFont, FontConfig, GenericFamilyMap, ResolvedFontConfig,
};
pub use geometry::{
    ArcParams, ArcToParams, CanvasColor, CanvasColorSpace, CanvasImageDataRef, CanvasPixelFormat,
    CornerRadius, CubicBezierParams, DirtyRect, EllipseParams, ImageCropParams, ImageDataSettings,
    QuadraticBezierParams, RadialGradientParams, RectParams, RoundRectParams,
};
pub use gradient::{CanvasGradient, GradientStop};
pub use path2d::Path2D;
pub use pattern::{CanvasPattern, Repetition};
pub use style::{
    CanvasFillRule, FontStretch, ImageSmoothingQuality, LineCap, LineJoin, TextAlign, TextBaseline,
};
