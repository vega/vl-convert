//! Canvas 2D rendering context implementation.

mod drawing;
mod image_ops;
mod path_ops;
mod text_rendering;
mod transform;

use crate::drawing_state::DrawingState;
use crate::error::{Canvas2dError, Canvas2dResult};
use crate::font_config::{font_config_to_fontdb, FontConfig, ResolvedFontConfig};
use crate::geometry::{CanvasColor, RadialGradientParams};
use crate::gradient::{CanvasGradient, GradientType};
use crate::pattern::{CanvasPattern, Repetition};
use crate::pattern_cache::PatternPixmapCache;
use crate::style::{
    CanvasFillRule, FillStyle, ImageSmoothingQuality, LineCap, LineJoin,
};
use cosmic_text::{FontSystem, SwashCache};
use std::sync::Arc;
use tiny_skia::Pixmap;

/// Maximum canvas dimension (same as Chrome).
const MAX_DIMENSION: u32 = 32767;

/// Maximum number of bytes retained by the per-context pattern pixmap cache.
const PATTERN_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

/// Canvas 2D rendering context.
pub struct Canvas2dContext {
    /// Width of the canvas in pixels.
    pub(crate) width: u32,
    /// Height of the canvas in pixels.
    pub(crate) height: u32,
    /// Pixel buffer.
    pub(crate) pixmap: Pixmap,
    /// Font system for text rendering.
    pub(crate) font_system: FontSystem,
    /// Swash cache for glyph rasterization.
    pub(crate) swash_cache: SwashCache,
    /// Current drawing state.
    pub(crate) state: DrawingState,
    /// Stack of saved drawing states.
    state_stack: Vec<DrawingState>,
    /// Fill rule associated with the current clipping path.
    pub(crate) clip_fill_rule: CanvasFillRule,
    /// Stack of saved clip fill rules (parallel to state_stack).
    clip_fill_rule_stack: Vec<CanvasFillRule>,
    /// Current path builder.
    pub(crate) path_builder: tiny_skia::PathBuilder,
    /// Current path position (for tracking subpath start).
    pub(crate) current_x: f32,
    pub(crate) current_y: f32,
    /// Subpath start position (for closePath).
    pub(crate) subpath_start_x: f32,
    pub(crate) subpath_start_y: f32,
    /// Whether the path has a current point (for arc/ellipse line_to vs move_to).
    pub(crate) has_current_point: bool,
    /// Owned cache of pattern backing pixmaps used for tiny-skia shader lifetimes.
    pub(crate) pattern_pixmap_cache: PatternPixmapCache,
    /// Whether font hinting is enabled for text rendering.
    pub(crate) hinting_enabled: bool,
}

impl Canvas2dContext {
    /// Create a new Canvas2dContext with the specified dimensions.
    ///
    /// Uses `FontConfig::default()` which loads system fonts and sets up
    /// standard generic family mappings (sans-serif, serif, monospace).
    pub fn new(width: u32, height: u32) -> Canvas2dResult<Self> {
        let config = FontConfig::default();
        let db = font_config_to_fontdb(&config);
        Self::new_internal(width, height, db, config.hinting_enabled)
    }

    /// Create a new Canvas2dContext with the specified dimensions and font configuration.
    pub fn with_config(width: u32, height: u32, config: FontConfig) -> Canvas2dResult<Self> {
        let db = font_config_to_fontdb(&config);
        Self::new_internal(width, height, db, config.hinting_enabled)
    }

    /// Create a new Canvas2dContext using a pre-resolved font configuration.
    ///
    /// This clones the cached font database from the [`ResolvedFontConfig`] rather
    /// than rebuilding it from scratch, avoiding repeated system font scanning.
    /// Use this when creating multiple canvas contexts that share the same fonts.
    pub fn with_resolved(
        width: u32,
        height: u32,
        resolved: &ResolvedFontConfig,
    ) -> Canvas2dResult<Self> {
        Self::new_internal(width, height, resolved.fontdb.clone(), resolved.hinting_enabled)
    }

    fn new_internal(width: u32, height: u32, font_db: fontdb::Database, hinting_enabled: bool) -> Canvas2dResult<Self> {
        // Validate dimensions
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(Canvas2dError::InvalidDimensions { width, height });
        }

        // Create pixmap
        let pixmap =
            Pixmap::new(width, height).ok_or(Canvas2dError::InvalidDimensions { width, height })?;

        // Create font system from the provided (already-configured) fontdb
        let font_system = FontSystem::new_with_locale_and_db("en".to_string(), font_db);

        // Create swash cache for glyph rasterization
        let swash_cache = SwashCache::new();

        Ok(Self {
            width,
            height,
            pixmap,
            font_system,
            swash_cache,
            state: DrawingState::default(),
            state_stack: Vec::new(),
            clip_fill_rule: CanvasFillRule::NonZero,
            clip_fill_rule_stack: Vec::new(),
            path_builder: tiny_skia::PathBuilder::new(),
            current_x: 0.0,
            current_y: 0.0,
            subpath_start_x: 0.0,
            subpath_start_y: 0.0,
            has_current_point: false,
            pattern_pixmap_cache: PatternPixmapCache::new(PATTERN_CACHE_MAX_BYTES),
            hinting_enabled,
        })
    }

    /// Get canvas width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get canvas height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Save the current drawing state.
    pub fn save(&mut self) {
        log::debug!(target: "canvas", "save");
        self.state_stack.push(self.state.clone());
        self.clip_fill_rule_stack.push(self.clip_fill_rule);
    }

    /// Restore the previously saved drawing state.
    pub fn restore(&mut self) {
        log::debug!(target: "canvas", "restore");
        if let Some(state) = self.state_stack.pop() {
            self.state = state;
            self.clip_fill_rule = self
                .clip_fill_rule_stack
                .pop()
                .unwrap_or(CanvasFillRule::NonZero);
        }
    }

    /// Reset the rendering context to its default state.
    ///
    /// This clears the canvas to transparent, resets all drawing state
    /// (fill/stroke style, transforms, etc.), and empties the state stack.
    pub fn reset(&mut self) {
        // Clear the canvas to transparent
        self.pixmap.fill(tiny_skia::Color::TRANSPARENT);

        // Reset state to defaults
        self.state = DrawingState::default();

        // Clear saved states
        self.state_stack.clear();
        self.clip_fill_rule_stack.clear();

        // Reset path
        self.path_builder = tiny_skia::PathBuilder::new();
        self.current_x = 0.0;
        self.current_y = 0.0;
        self.subpath_start_x = 0.0;
        self.subpath_start_y = 0.0;
        self.has_current_point = false;
        self.clip_fill_rule = CanvasFillRule::NonZero;

        // Reset pattern cache so stale pattern backing pixmaps are released.
        self.pattern_pixmap_cache.clear();
    }

    // --- Style setters ---

    /// Set the fill style from a CSS color string.
    pub fn set_fill_style(&mut self, style: &str) -> Canvas2dResult<()> {
        let color = parse_color(style)?;
        self.state.fill_style = FillStyle::Color(color);
        Ok(())
    }

    /// Set the fill style from a CanvasColor.
    pub fn set_fill_style_color(&mut self, color: CanvasColor) {
        self.state.fill_style = FillStyle::Color(color.into());
    }

    /// Set the stroke style from a CSS color string.
    pub fn set_stroke_style(&mut self, style: &str) -> Canvas2dResult<()> {
        let color = parse_color(style)?;
        self.state.stroke_style = FillStyle::Color(color);
        Ok(())
    }

    /// Set the stroke style from a CanvasColor.
    pub fn set_stroke_style_color(&mut self, color: CanvasColor) {
        self.state.stroke_style = FillStyle::Color(color.into());
    }

    /// Set the line width.
    /// Per spec: ignore non-finite or values <= 0.
    pub fn set_line_width(&mut self, width: f32) {
        if width.is_finite() && width > 0.0 {
            self.state.line_width = width;
        }
    }

    /// Set the line cap style.
    pub fn set_line_cap(&mut self, cap: LineCap) {
        self.state.line_cap = cap;
    }

    /// Set the line join style.
    pub fn set_line_join(&mut self, join: LineJoin) {
        self.state.line_join = join;
    }

    /// Set the miter limit.
    /// Per spec: ignore non-finite or values <= 0.
    pub fn set_miter_limit(&mut self, limit: f32) {
        if limit.is_finite() && limit > 0.0 {
            self.state.miter_limit = limit;
        }
    }

    /// Set the global alpha (opacity).
    /// Per spec: ignore non-finite or values outside [0.0, 1.0].
    pub fn set_global_alpha(&mut self, alpha: f32) {
        if alpha.is_finite() && (0.0..=1.0).contains(&alpha) {
            self.state.global_alpha = alpha;
        }
    }

    /// Set the global composite operation (blend mode).
    /// Per spec: ignore invalid values, preserve previous mode.
    /// Returns true if the value was accepted.
    pub fn set_global_composite_operation(&mut self, op: &str) -> bool {
        let mode = match op {
            "source-over" => tiny_skia::BlendMode::SourceOver,
            "source-in" => tiny_skia::BlendMode::SourceIn,
            "source-out" => tiny_skia::BlendMode::SourceOut,
            "source-atop" => tiny_skia::BlendMode::SourceAtop,
            "destination-over" => tiny_skia::BlendMode::DestinationOver,
            "destination-in" => tiny_skia::BlendMode::DestinationIn,
            "destination-out" => tiny_skia::BlendMode::DestinationOut,
            "destination-atop" => tiny_skia::BlendMode::DestinationAtop,
            "lighter" => tiny_skia::BlendMode::Plus,
            "copy" => tiny_skia::BlendMode::Source,
            "xor" => tiny_skia::BlendMode::Xor,
            "multiply" => tiny_skia::BlendMode::Multiply,
            "screen" => tiny_skia::BlendMode::Screen,
            "overlay" => tiny_skia::BlendMode::Overlay,
            "darken" => tiny_skia::BlendMode::Darken,
            "lighten" => tiny_skia::BlendMode::Lighten,
            "color-dodge" => tiny_skia::BlendMode::ColorDodge,
            "color-burn" => tiny_skia::BlendMode::ColorBurn,
            "hard-light" => tiny_skia::BlendMode::HardLight,
            "soft-light" => tiny_skia::BlendMode::SoftLight,
            "difference" => tiny_skia::BlendMode::Difference,
            "exclusion" => tiny_skia::BlendMode::Exclusion,
            "hue" => tiny_skia::BlendMode::Hue,
            "saturation" => tiny_skia::BlendMode::Saturation,
            "color" => tiny_skia::BlendMode::Color,
            "luminosity" => tiny_skia::BlendMode::Luminosity,
            _ => return false,
        };
        self.state.global_composite_operation = mode;
        true
    }

    /// Set the line dash pattern.
    /// Per spec: ignore if any value is non-finite or negative.
    /// Duplicate odd-length arrays to make them even.
    pub fn set_line_dash(&mut self, mut segments: Vec<f32>) {
        // Reject if any value is non-finite or negative
        if segments.iter().any(|&v| !v.is_finite() || v < 0.0) {
            return;
        }
        // Duplicate odd-length arrays per spec
        if !segments.len().is_multiple_of(2) {
            let copy = segments.clone();
            segments.extend(copy);
        }
        self.state.line_dash = segments;
    }

    /// Get the current line dash pattern.
    pub fn get_line_dash(&self) -> &[f32] {
        &self.state.line_dash
    }

    /// Set the line dash offset.
    /// Per spec: ignore non-finite values.
    pub fn set_line_dash_offset(&mut self, offset: f32) {
        if offset.is_finite() {
            self.state.line_dash_offset = offset;
        }
    }

    // --- Image smoothing ---

    /// Set whether image smoothing is enabled.
    pub fn set_image_smoothing_enabled(&mut self, enabled: bool) {
        self.state.image_smoothing_enabled = enabled;
    }

    /// Get whether image smoothing is enabled.
    pub fn get_image_smoothing_enabled(&self) -> bool {
        self.state.image_smoothing_enabled
    }

    /// Set the image smoothing quality.
    pub fn set_image_smoothing_quality(&mut self, quality: ImageSmoothingQuality) {
        self.state.image_smoothing_quality = quality;
    }

    /// Get the image smoothing quality.
    pub fn get_image_smoothing_quality(&self) -> ImageSmoothingQuality {
        self.state.image_smoothing_quality
    }

    /// Get the filter quality for image rendering based on smoothing settings.
    pub(crate) fn get_image_filter_quality(&self) -> tiny_skia::FilterQuality {
        if self.state.image_smoothing_enabled {
            self.state.image_smoothing_quality.into()
        } else {
            tiny_skia::FilterQuality::Nearest
        }
    }

    // --- Gradients ---

    /// Create a linear gradient.
    pub fn create_linear_gradient(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> CanvasGradient {
        CanvasGradient::new_linear(x0, y0, x1, y1)
    }

    /// Create a radial gradient.
    pub fn create_radial_gradient(&self, params: &RadialGradientParams) -> CanvasGradient {
        CanvasGradient::new_radial(params)
    }

    /// Set the fill style to a gradient.
    pub fn set_fill_style_gradient(&mut self, gradient: CanvasGradient) {
        match gradient.gradient_type {
            GradientType::Linear { .. } => {
                self.state.fill_style = FillStyle::LinearGradient(gradient);
            }
            GradientType::Radial { .. } => {
                self.state.fill_style = FillStyle::RadialGradient(gradient);
            }
        }
    }

    /// Set the stroke style to a gradient.
    pub fn set_stroke_style_gradient(&mut self, gradient: CanvasGradient) {
        match gradient.gradient_type {
            GradientType::Linear { .. } => {
                self.state.stroke_style = FillStyle::LinearGradient(gradient);
            }
            GradientType::Radial { .. } => {
                self.state.stroke_style = FillStyle::RadialGradient(gradient);
            }
        }
    }

    // --- Patterns ---

    /// Create a pattern from RGBA pixel data.
    ///
    /// # Arguments
    /// * `data` - RGBA pixel data (4 bytes per pixel, non-premultiplied)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `repetition` - Repetition mode string: "repeat", "repeat-x", "repeat-y", or "no-repeat"
    pub fn create_pattern(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        repetition: &str,
    ) -> Canvas2dResult<Arc<CanvasPattern>> {
        let rep = repetition.parse::<Repetition>()?;
        let pattern = CanvasPattern::new(data, width, height, rep)?;
        Ok(Arc::new(pattern))
    }

    /// Set the fill style to a pattern.
    pub fn set_fill_style_pattern(&mut self, pattern: Arc<CanvasPattern>) {
        self.state.fill_style = FillStyle::Pattern(pattern);
    }

    /// Set the stroke style to a pattern.
    pub fn set_stroke_style_pattern(&mut self, pattern: Arc<CanvasPattern>) {
        self.state.stroke_style = FillStyle::Pattern(pattern);
    }

    #[cfg(test)]
    fn pattern_cache_entry_count(&self) -> usize {
        self.pattern_pixmap_cache.len()
    }

    #[cfg(test)]
    fn pattern_cache_total_bytes(&self) -> usize {
        self.pattern_pixmap_cache.total_bytes()
    }
}

/// Parse a CSS color string into a tiny_skia::Color.
pub(crate) fn parse_color(s: &str) -> Canvas2dResult<tiny_skia::Color> {
    let parsed = csscolorparser::parse(s)
        .map_err(|e| Canvas2dError::ColorParseError(format!("{}: {}", s, e)))?;

    let [r, g, b, a] = parsed.to_array();
    Ok(tiny_skia::Color::from_rgba(r, g, b, a).unwrap_or(tiny_skia::Color::BLACK))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom_matrix::DOMMatrix;
    use crate::geometry::{ArcToParams, RectParams};
    use crate::pattern_cache::PatternPixmapCache;
    use crate::style::FontStretch;

    #[test]
    fn test_new_context_defaults() {
        let ctx = Canvas2dContext::new(200, 150).unwrap();
        assert_eq!(ctx.width(), 200);
        assert_eq!(ctx.height(), 150);
        // Default drawing state
        assert_eq!(ctx.state.line_width, 1.0);
        assert_eq!(ctx.state.global_alpha, 1.0);
        assert_eq!(ctx.state.miter_limit, 10.0);
        assert!(ctx.state.line_dash.is_empty());
        assert_eq!(ctx.state.line_dash_offset, 0.0);
        assert!(ctx.state.image_smoothing_enabled);
        assert!(ctx.state.clip_path.is_none());
        assert_eq!(ctx.clip_fill_rule, CanvasFillRule::NonZero);
        // Canvas should be fully transparent
        assert!(ctx.pixmap.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_invalid_dimensions() {
        assert!(matches!(
            Canvas2dContext::new(0, 100),
            Err(Canvas2dError::InvalidDimensions { .. })
        ));
        assert!(matches!(
            Canvas2dContext::new(100, 0),
            Err(Canvas2dError::InvalidDimensions { .. })
        ));
    }

    #[test]
    fn test_line_width_ignore_invalid() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_line_width(5.0);
        assert_eq!(ctx.state.line_width, 5.0);

        // Negative values are ignored (value preserved)
        ctx.set_line_width(-1.0);
        assert_eq!(ctx.state.line_width, 5.0);

        // Zero is ignored
        ctx.set_line_width(0.0);
        assert_eq!(ctx.state.line_width, 5.0);

        // Non-finite values are ignored
        ctx.set_line_width(f32::NAN);
        assert_eq!(ctx.state.line_width, 5.0);
        ctx.set_line_width(f32::INFINITY);
        assert_eq!(ctx.state.line_width, 5.0);
        ctx.set_line_width(f32::NEG_INFINITY);
        assert_eq!(ctx.state.line_width, 5.0);

        // Valid positive values are accepted
        ctx.set_line_width(3.0);
        assert_eq!(ctx.state.line_width, 3.0);
    }

    #[test]
    fn test_line_cap_and_join() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();

        ctx.set_line_cap(LineCap::Round);
        assert_eq!(ctx.state.line_cap, LineCap::Round);

        ctx.set_line_join(LineJoin::Bevel);
        assert_eq!(ctx.state.line_join, LineJoin::Bevel);
    }

    #[test]
    fn test_line_dash_set_get() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        // Even-length arrays stored as-is
        ctx.set_line_dash(vec![5.0, 10.0]);
        assert_eq!(ctx.get_line_dash(), &[5.0, 10.0]);

        // Odd-length arrays are duplicated per spec
        ctx.set_line_dash(vec![5.0, 10.0, 15.0]);
        assert_eq!(ctx.get_line_dash(), &[5.0, 10.0, 15.0, 5.0, 10.0, 15.0]);

        ctx.set_line_dash_offset(3.5);
        assert_eq!(ctx.state.line_dash_offset, 3.5);
    }

    #[test]
    fn test_line_dash_ignore_invalid() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_line_dash(vec![5.0, 5.0]);
        assert_eq!(ctx.get_line_dash(), &[5.0, 5.0]);

        // Negative values cause entire call to be ignored
        ctx.set_line_dash(vec![5.0, -1.0]);
        assert_eq!(ctx.get_line_dash(), &[5.0, 5.0]);

        // NaN causes entire call to be ignored
        ctx.set_line_dash(vec![5.0, f32::NAN]);
        assert_eq!(ctx.get_line_dash(), &[5.0, 5.0]);

        // Infinity causes entire call to be ignored
        ctx.set_line_dash(vec![f32::INFINITY, 5.0]);
        assert_eq!(ctx.get_line_dash(), &[5.0, 5.0]);

        // Empty array is valid (clears dash)
        ctx.set_line_dash(vec![]);
        assert!(ctx.get_line_dash().is_empty());

        // Single element (odd) is duplicated
        ctx.set_line_dash(vec![3.0]);
        assert_eq!(ctx.get_line_dash(), &[3.0, 3.0]);
    }

    #[test]
    fn test_line_dash_offset_ignore_invalid() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_line_dash_offset(5.0);
        assert_eq!(ctx.state.line_dash_offset, 5.0);

        // Non-finite values are ignored
        ctx.set_line_dash_offset(f32::NAN);
        assert_eq!(ctx.state.line_dash_offset, 5.0);
        ctx.set_line_dash_offset(f32::INFINITY);
        assert_eq!(ctx.state.line_dash_offset, 5.0);
        ctx.set_line_dash_offset(f32::NEG_INFINITY);
        assert_eq!(ctx.state.line_dash_offset, 5.0);

        // Valid values are accepted (including negative and zero)
        ctx.set_line_dash_offset(-2.0);
        assert_eq!(ctx.state.line_dash_offset, -2.0);
        ctx.set_line_dash_offset(0.0);
        assert_eq!(ctx.state.line_dash_offset, 0.0);
    }

    #[test]
    fn test_global_alpha_ignore_invalid() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_global_alpha(0.5);
        assert_eq!(ctx.state.global_alpha, 0.5);

        // Out-of-range values are ignored (not clamped)
        ctx.set_global_alpha(2.0);
        assert_eq!(ctx.state.global_alpha, 0.5);
        ctx.set_global_alpha(-0.5);
        assert_eq!(ctx.state.global_alpha, 0.5);

        // Non-finite values are ignored
        ctx.set_global_alpha(f32::NAN);
        assert_eq!(ctx.state.global_alpha, 0.5);
        ctx.set_global_alpha(f32::INFINITY);
        assert_eq!(ctx.state.global_alpha, 0.5);
        ctx.set_global_alpha(f32::NEG_INFINITY);
        assert_eq!(ctx.state.global_alpha, 0.5);

        // Valid boundary values are accepted
        ctx.set_global_alpha(0.0);
        assert_eq!(ctx.state.global_alpha, 0.0);
        ctx.set_global_alpha(1.0);
        assert_eq!(ctx.state.global_alpha, 1.0);
    }

    #[test]
    fn test_miter_limit_ignore_invalid() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        assert_eq!(ctx.state.miter_limit, 10.0); // default

        ctx.set_miter_limit(5.0);
        assert_eq!(ctx.state.miter_limit, 5.0);

        // Non-positive values are ignored
        ctx.set_miter_limit(0.0);
        assert_eq!(ctx.state.miter_limit, 5.0);
        ctx.set_miter_limit(-1.0);
        assert_eq!(ctx.state.miter_limit, 5.0);

        // Non-finite values are ignored
        ctx.set_miter_limit(f32::NAN);
        assert_eq!(ctx.state.miter_limit, 5.0);
        ctx.set_miter_limit(f32::INFINITY);
        assert_eq!(ctx.state.miter_limit, 5.0);

        // Valid positive values are accepted
        ctx.set_miter_limit(2.0);
        assert_eq!(ctx.state.miter_limit, 2.0);
    }

    #[test]
    fn test_global_composite_operation_ignore_invalid() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        // Default is source-over
        assert_eq!(
            ctx.state.global_composite_operation,
            tiny_skia::BlendMode::SourceOver
        );

        // Valid value is accepted
        assert!(ctx.set_global_composite_operation("multiply"));
        assert_eq!(
            ctx.state.global_composite_operation,
            tiny_skia::BlendMode::Multiply
        );

        // Invalid value is ignored, previous mode preserved
        assert!(!ctx.set_global_composite_operation("invalid-mode"));
        assert_eq!(
            ctx.state.global_composite_operation,
            tiny_skia::BlendMode::Multiply
        );

        // Empty string is invalid
        assert!(!ctx.set_global_composite_operation(""));
        assert_eq!(
            ctx.state.global_composite_operation,
            tiny_skia::BlendMode::Multiply
        );
    }

    #[test]
    fn test_save_restore_line_state() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();

        // Set non-default line state
        ctx.set_line_width(5.0);
        ctx.set_line_cap(LineCap::Round);
        ctx.set_line_join(LineJoin::Bevel);
        ctx.set_line_dash(vec![4.0, 2.0]);
        ctx.set_line_dash_offset(1.5);
        ctx.set_global_alpha(0.7);
        ctx.save();

        // Modify everything
        ctx.set_line_width(10.0);
        ctx.set_line_cap(LineCap::Square);
        ctx.set_line_join(LineJoin::Round);
        ctx.set_line_dash(vec![1.0]);
        ctx.set_line_dash_offset(0.0);
        ctx.set_global_alpha(0.3);

        ctx.restore();

        // All values should be restored
        assert_eq!(ctx.state.line_width, 5.0);
        assert_eq!(ctx.state.line_cap, LineCap::Round);
        assert_eq!(ctx.state.line_join, LineJoin::Bevel);
        assert_eq!(ctx.get_line_dash(), &[4.0, 2.0]);
        assert_eq!(ctx.state.line_dash_offset, 1.5);
        assert_eq!(ctx.state.global_alpha, 0.7);
    }

    #[test]
    fn test_save_restore_transform() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.translate(10.0, 20.0);
        ctx.save();
        ctx.translate(30.0, 40.0);

        let t = ctx.get_transform();
        assert_eq!(t.e, 40.0); // 10 + 30
        assert_eq!(t.f, 60.0); // 20 + 40

        ctx.restore();
        let t = ctx.get_transform();
        assert_eq!(t.e, 10.0);
        assert_eq!(t.f, 20.0);
    }

    #[test]
    fn test_fill_rect_pixels() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_fill_style("#ff0000").unwrap();
        ctx.fill_rect(&RectParams {
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 50.0,
        });

        let data = ctx.get_image_data(0, 0, 100, 100);
        // Inside the rect at (30, 30): should be red
        let idx = (30 * 100 + 30) * 4;
        assert_eq!(data[idx], 255); // R
        assert_eq!(data[idx + 1], 0); // G
        assert_eq!(data[idx + 2], 0); // B
        assert_eq!(data[idx + 3], 255); // A

        // Outside the rect at (5, 5): should be transparent
        let idx_out = (5 * 100 + 5) * 4;
        assert_eq!(data[idx_out + 3], 0); // A
    }

    #[test]
    fn test_stroke_rect_pixels() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_stroke_style("#0000ff").unwrap();
        ctx.set_line_width(2.0);
        ctx.stroke_rect(&RectParams {
            x: 20.0,
            y: 20.0,
            width: 60.0,
            height: 60.0,
        });

        let data = ctx.get_image_data(0, 0, 100, 100);
        // On the top edge at (50, 20): should have blue pixels
        let idx = (20 * 100 + 50) * 4;
        assert!(data[idx + 2] > 200); // B channel
        assert!(data[idx + 3] > 0); // A

        // Center of rect (50, 50): should be transparent (stroke only)
        let idx_center = (50 * 100 + 50) * 4;
        assert_eq!(data[idx_center + 3], 0);
    }

    #[test]
    fn test_reset() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();

        ctx.set_fill_style("#ff0000").unwrap();
        ctx.set_line_width(5.0);
        ctx.set_global_alpha(0.5);
        ctx.translate(10.0, 10.0);
        ctx.save();
        ctx.fill_rect(&RectParams {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        });

        // Populate pattern cache and verify reset clears it.
        let pattern_data = vec![255_u8; 8 * 8 * 4];
        let pattern = ctx.create_pattern(&pattern_data, 8, 8, "repeat").unwrap();
        ctx.set_fill_style_pattern(pattern);
        ctx.fill_rect(&RectParams {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });

        assert!(ctx.pixmap.data().iter().any(|&b| b != 0));
        assert!(ctx.pattern_cache_entry_count() > 0);

        ctx.reset();

        // Canvas should be clear
        assert!(ctx.pixmap.data().iter().all(|&b| b == 0));
        assert_eq!(ctx.pattern_cache_entry_count(), 0);
        assert_eq!(ctx.pattern_cache_total_bytes(), 0);
        // State should be back to defaults
        assert_eq!(ctx.state.line_width, 1.0);
        assert_eq!(ctx.state.global_alpha, 1.0);
        let t = ctx.get_transform();
        assert_eq!(t.a, 1.0);
        assert_eq!(t.d, 1.0);
        assert_eq!(t.e, 0.0);
        assert_eq!(t.f, 0.0);
        assert_eq!(ctx.clip_fill_rule, CanvasFillRule::NonZero);
    }

    #[test]
    fn test_clip_fill_rule_save_restore_and_reset() {
        let mut ctx = Canvas2dContext::new(64, 64).unwrap();

        ctx.begin_path();
        ctx.rect(&RectParams {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        });
        ctx.clip_with_rule(CanvasFillRule::EvenOdd);
        assert_eq!(ctx.clip_fill_rule, CanvasFillRule::EvenOdd);

        ctx.save();

        ctx.begin_path();
        ctx.rect(&RectParams {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        });
        ctx.clip_with_rule(CanvasFillRule::NonZero);
        assert_eq!(ctx.clip_fill_rule, CanvasFillRule::NonZero);

        ctx.restore();
        assert_eq!(ctx.clip_fill_rule, CanvasFillRule::EvenOdd);

        ctx.reset();
        assert_eq!(ctx.clip_fill_rule, CanvasFillRule::NonZero);
    }

    #[test]
    fn test_arc_to_with_non_invertible_transform() {
        // With unified user-space coordinates, arc_to works in user space
        // regardless of transform. A non-invertible transform just means
        // the path collapses at render time, but path building should not panic.
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.begin_path();
        ctx.move_to(10.0, 10.0);
        ctx.set_transform(DOMMatrix::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0));

        ctx.arc_to(&ArcToParams {
            x1: 30.0,
            y1: 10.0,
            x2: 30.0,
            y2: 30.0,
            radius: 12.0,
        });

        assert!(ctx.has_current_point);
    }

    #[test]
    fn test_create_image_data() {
        let ctx = Canvas2dContext::new(100, 100).unwrap();
        let data = ctx.create_image_data(50, 30);
        assert_eq!(data.len(), 50 * 30 * 4);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_create_image_data_large() {
        let ctx = Canvas2dContext::new(100, 100).unwrap();
        let data = ctx.create_image_data(1000, 1000);
        assert_eq!(data.len(), 1000 * 1000 * 4);
    }

    #[test]
    fn test_font_stretch_set_get() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        assert_eq!(ctx.get_font_stretch(), FontStretch::Normal);

        ctx.set_font_stretch(FontStretch::Condensed);
        assert_eq!(ctx.get_font_stretch(), FontStretch::Condensed);

        ctx.set_font_stretch(FontStretch::UltraExpanded);
        assert_eq!(ctx.get_font_stretch(), FontStretch::UltraExpanded);
    }

    #[test]
    fn test_font_stretch_via_font_setter() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_font("condensed 12px Arial").unwrap();
        assert_eq!(ctx.get_font_stretch(), FontStretch::Condensed);

        // Setting font without stretch resets to Normal
        ctx.set_font("12px Arial").unwrap();
        assert_eq!(ctx.get_font_stretch(), FontStretch::Normal);
    }

    #[test]
    fn test_font_stretch_save_restore() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_font_stretch(FontStretch::SemiExpanded);
        ctx.save();

        ctx.set_font_stretch(FontStretch::ExtraCondensed);
        assert_eq!(ctx.get_font_stretch(), FontStretch::ExtraCondensed);

        ctx.restore();
        assert_eq!(ctx.get_font_stretch(), FontStretch::SemiExpanded);
    }

    #[test]
    fn test_font_stretch_reset() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_font_stretch(FontStretch::Expanded);
        ctx.reset();
        assert_eq!(ctx.get_font_stretch(), FontStretch::Normal);
    }

    #[test]
    fn test_pattern_cache_lru_eviction() {
        let mut ctx = Canvas2dContext::new(64, 64).unwrap();
        ctx.pattern_pixmap_cache = PatternPixmapCache::new(256);

        let p1_data = vec![255_u8; 8 * 8 * 4];
        let p1 = ctx.create_pattern(&p1_data, 8, 8, "repeat").unwrap();
        ctx.set_fill_style_pattern(p1);
        ctx.fill_rect(&RectParams {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        });
        assert_eq!(ctx.pattern_cache_entry_count(), 1);
        assert_eq!(ctx.pattern_cache_total_bytes(), 256);

        let p2_data = vec![64_u8; 8 * 8 * 4];
        let p2 = ctx.create_pattern(&p2_data, 8, 8, "repeat").unwrap();
        ctx.set_fill_style_pattern(p2);
        ctx.fill_rect(&RectParams {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        });

        // Cache budget only allows one 8x8 pixmap, so inserting p2 evicts p1.
        assert_eq!(ctx.pattern_cache_entry_count(), 1);
        assert_eq!(ctx.pattern_cache_total_bytes(), 256);
    }
}
