//! Style types and enums for Canvas 2D operations.

use crate::gradient::CanvasGradient;

/// Fill style for Canvas 2D operations.
#[derive(Debug, Clone)]
pub enum FillStyle {
    /// Solid color fill.
    Color(tiny_skia::Color),
    /// Linear gradient fill.
    LinearGradient(CanvasGradient),
    /// Radial gradient fill.
    RadialGradient(CanvasGradient),
    /// Pattern fill (not yet implemented).
    Pattern,
}

impl Default for FillStyle {
    fn default() -> Self {
        // Default is opaque black
        FillStyle::Color(tiny_skia::Color::BLACK)
    }
}

/// Line cap style for stroke operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Flat edge at the endpoint.
    #[default]
    Butt,
    /// Rounded edge extending past the endpoint.
    Round,
    /// Square edge extending past the endpoint.
    Square,
}

impl From<LineCap> for tiny_skia::LineCap {
    fn from(cap: LineCap) -> Self {
        match cap {
            LineCap::Butt => tiny_skia::LineCap::Butt,
            LineCap::Round => tiny_skia::LineCap::Round,
            LineCap::Square => tiny_skia::LineCap::Square,
        }
    }
}

/// Line join style for stroke operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// Sharp corner.
    #[default]
    Miter,
    /// Rounded corner.
    Round,
    /// Beveled corner.
    Bevel,
}

impl From<LineJoin> for tiny_skia::LineJoin {
    fn from(join: LineJoin) -> Self {
        match join {
            LineJoin::Miter => tiny_skia::LineJoin::Miter,
            LineJoin::Round => tiny_skia::LineJoin::Round,
            LineJoin::Bevel => tiny_skia::LineJoin::Bevel,
        }
    }
}

/// Text alignment for text rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Align text to the left of the anchor point.
    #[default]
    Left,
    /// Align text to the right of the anchor point.
    Right,
    /// Center text on the anchor point.
    Center,
    /// Align text to the start (left for LTR, right for RTL).
    Start,
    /// Align text to the end (right for LTR, left for RTL).
    End,
}

/// Text baseline for text rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextBaseline {
    /// Top of the em square.
    Top,
    /// Hanging baseline.
    Hanging,
    /// Middle of the em square.
    Middle,
    /// Alphabetic baseline.
    #[default]
    Alphabetic,
    /// Ideographic baseline.
    Ideographic,
    /// Bottom of the em square.
    Bottom,
}

/// Fill rule for path operations.
///
/// Determines how the interior of a path is calculated when filling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CanvasFillRule {
    /// Non-zero winding rule (default).
    #[default]
    NonZero,
    /// Even-odd rule.
    EvenOdd,
}

impl From<CanvasFillRule> for tiny_skia::FillRule {
    fn from(rule: CanvasFillRule) -> Self {
        match rule {
            CanvasFillRule::NonZero => tiny_skia::FillRule::Winding,
            CanvasFillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
        }
    }
}

/// Image smoothing quality levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageSmoothingQuality {
    /// Low quality (fastest).
    Low,
    /// Medium quality (balanced).
    #[default]
    Medium,
    /// High quality (slowest).
    High,
}

impl From<ImageSmoothingQuality> for tiny_skia::FilterQuality {
    fn from(quality: ImageSmoothingQuality) -> Self {
        match quality {
            ImageSmoothingQuality::Low => tiny_skia::FilterQuality::Bilinear,
            ImageSmoothingQuality::Medium => tiny_skia::FilterQuality::Bilinear,
            ImageSmoothingQuality::High => tiny_skia::FilterQuality::Bicubic,
        }
    }
}
