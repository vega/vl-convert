//! Drawing state that can be saved and restored.

use crate::font_parser::ParsedFont;
use crate::style::{FillStyle, ImageSmoothingQuality, LineCap, LineJoin, TextAlign, TextBaseline};
use tiny_skia::Transform;

/// Drawing state that can be saved and restored.
#[derive(Debug, Clone)]
pub struct DrawingState {
    /// Current fill style.
    pub fill_style: FillStyle,
    /// Current stroke style.
    pub stroke_style: FillStyle,
    /// Current line width.
    pub line_width: f32,
    /// Current line cap style.
    pub line_cap: LineCap,
    /// Current line join style.
    pub line_join: LineJoin,
    /// Current miter limit.
    pub miter_limit: f32,
    /// Current line dash pattern.
    pub line_dash: Vec<f32>,
    /// Current line dash offset.
    pub line_dash_offset: f32,
    /// Current font specification.
    pub font: ParsedFont,
    /// Current text alignment.
    pub text_align: TextAlign,
    /// Current text baseline.
    pub text_baseline: TextBaseline,
    /// Current global alpha.
    pub global_alpha: f32,
    /// Current global composite operation (blend mode).
    pub global_composite_operation: tiny_skia::BlendMode,
    /// Current transform matrix.
    pub transform: Transform,
    /// Clipping path (if any).
    pub clip_path: Option<tiny_skia::Path>,
    /// Transform that was active when the clip path was set.
    /// Used to transform the user-space clip path into device space at mask creation time.
    pub clip_transform: Transform,
    /// Letter spacing for text rendering (in pixels).
    pub letter_spacing: f32,
    /// Whether image smoothing is enabled.
    pub image_smoothing_enabled: bool,
    /// Image smoothing quality level.
    pub image_smoothing_quality: ImageSmoothingQuality,
}

impl Default for DrawingState {
    fn default() -> Self {
        Self {
            fill_style: FillStyle::default(),
            stroke_style: FillStyle::default(),
            line_width: 1.0,
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            miter_limit: 10.0,
            line_dash: Vec::new(),
            line_dash_offset: 0.0,
            font: ParsedFont::default(),
            text_align: TextAlign::default(),
            text_baseline: TextBaseline::default(),
            global_alpha: 1.0,
            global_composite_operation: tiny_skia::BlendMode::SourceOver,
            transform: Transform::identity(),
            clip_path: None,
            clip_transform: Transform::identity(),
            letter_spacing: 0.0,
            image_smoothing_enabled: true,
            image_smoothing_quality: ImageSmoothingQuality::default(),
        }
    }
}
