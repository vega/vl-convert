//! Style types and enums for Canvas 2D operations.

use crate::gradient::CanvasGradient;
use crate::pattern::CanvasPattern;
use std::sync::Arc;

/// Fill style for Canvas 2D operations.
#[derive(Debug, Clone)]
pub enum FillStyle {
    /// Solid color fill.
    Color(tiny_skia::Color),
    /// Linear gradient fill.
    LinearGradient(CanvasGradient),
    /// Radial gradient fill.
    RadialGradient(CanvasGradient),
    /// Pattern fill.
    Pattern(Arc<CanvasPattern>),
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
    Left,
    /// Align text to the right of the anchor point.
    Right,
    /// Center text on the anchor point.
    Center,
    /// Align text to the start (left for LTR, right for RTL).
    #[default]
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

/// Font stretch (width) for text rendering.
///
/// Maps to CSS `font-stretch` keywords and `cosmic_text::Stretch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStretch {
    /// Ultra-condensed width.
    UltraCondensed,
    /// Extra-condensed width.
    ExtraCondensed,
    /// Condensed width.
    Condensed,
    /// Semi-condensed width.
    SemiCondensed,
    /// Normal width.
    #[default]
    Normal,
    /// Semi-expanded width.
    SemiExpanded,
    /// Expanded width.
    Expanded,
    /// Extra-expanded width.
    ExtraExpanded,
    /// Ultra-expanded width.
    UltraExpanded,
}

impl FontStretch {
    /// Convert from a CSS keyword string. Returns None for invalid values.
    pub fn from_css_keyword(s: &str) -> Option<Self> {
        match s {
            "ultra-condensed" => Some(Self::UltraCondensed),
            "extra-condensed" => Some(Self::ExtraCondensed),
            "condensed" => Some(Self::Condensed),
            "semi-condensed" => Some(Self::SemiCondensed),
            "normal" => Some(Self::Normal),
            "semi-expanded" => Some(Self::SemiExpanded),
            "expanded" => Some(Self::Expanded),
            "extra-expanded" => Some(Self::ExtraExpanded),
            "ultra-expanded" => Some(Self::UltraExpanded),
            _ => None,
        }
    }

    /// Convert to CSS keyword string.
    pub fn as_css_keyword(&self) -> &'static str {
        match self {
            Self::UltraCondensed => "ultra-condensed",
            Self::ExtraCondensed => "extra-condensed",
            Self::Condensed => "condensed",
            Self::SemiCondensed => "semi-condensed",
            Self::Normal => "normal",
            Self::SemiExpanded => "semi-expanded",
            Self::Expanded => "expanded",
            Self::ExtraExpanded => "extra-expanded",
            Self::UltraExpanded => "ultra-expanded",
        }
    }
}

impl From<FontStretch> for cosmic_text::Stretch {
    fn from(stretch: FontStretch) -> Self {
        match stretch {
            FontStretch::UltraCondensed => cosmic_text::Stretch::UltraCondensed,
            FontStretch::ExtraCondensed => cosmic_text::Stretch::ExtraCondensed,
            FontStretch::Condensed => cosmic_text::Stretch::Condensed,
            FontStretch::SemiCondensed => cosmic_text::Stretch::SemiCondensed,
            FontStretch::Normal => cosmic_text::Stretch::Normal,
            FontStretch::SemiExpanded => cosmic_text::Stretch::SemiExpanded,
            FontStretch::Expanded => cosmic_text::Stretch::Expanded,
            FontStretch::ExtraExpanded => cosmic_text::Stretch::ExtraExpanded,
            FontStretch::UltraExpanded => cosmic_text::Stretch::UltraExpanded,
        }
    }
}

/// Image smoothing quality levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageSmoothingQuality {
    /// Low quality (fastest).
    #[default]
    Low,
    /// Medium quality (balanced).
    Medium,
    /// High quality (slowest).
    High,
}

impl From<ImageSmoothingQuality> for tiny_skia::FilterQuality {
    fn from(quality: ImageSmoothingQuality) -> Self {
        match quality {
            // Low uses Nearest for actual performance benefit
            ImageSmoothingQuality::Low => tiny_skia::FilterQuality::Nearest,
            ImageSmoothingQuality::Medium => tiny_skia::FilterQuality::Bilinear,
            ImageSmoothingQuality::High => tiny_skia::FilterQuality::Bicubic,
        }
    }
}
