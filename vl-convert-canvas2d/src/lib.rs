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
//! use vl_convert_canvas2d::Canvas2dContext;
//!
//! let mut ctx = Canvas2dContext::new(400, 300)?;
//! ctx.set_fill_style("#ff0000");
//! ctx.fill_rect(10.0, 10.0, 100.0, 50.0);
//! let png_data = ctx.to_png()?;
//! ```

mod arc;
mod context;
mod error;
mod font_parser;
mod geometry;
mod gradient;
mod path2d;
mod pattern;
mod style;
mod text;

// Re-export public API
pub use context::{Canvas2dContext, Canvas2dContextBuilder, DOMMatrix, DrawingState};
pub use error::{Canvas2dError, Canvas2dResult};
pub use geometry::{
    ArcParams, ArcToParams, CubicBezierParams, DirtyRect, EllipseParams, ImageCropParams,
    RadialGradientParams,
};
pub use gradient::{CanvasGradient, GradientStop};
pub use path2d::Path2D;
pub use pattern::{CanvasPattern, Repetition};
pub use style::{
    CanvasFillRule, FillStyle, ImageSmoothingQuality, LineCap, LineJoin, TextAlign, TextBaseline,
};
