//! Canvas 2D rendering context implementation.

use crate::error::{Canvas2dError, Canvas2dResult};
use crate::font_parser::{parse_font, ParsedFont};
use crate::gradient::{CanvasGradient, GradientType};
use crate::path2d::Path2D;
use crate::pattern::{CanvasPattern, Repetition};
use crate::style::{
    CanvasFillRule, FillStyle, ImageSmoothingQuality, LineCap, LineJoin, TextAlign, TextBaseline,
};
use crate::text::TextMetrics;
use cosmic_text::{Attrs, Buffer, Command, Family, FontSystem, Metrics, Shaping, SwashCache};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

/// Maximum canvas dimension (same as Chrome).
const MAX_DIMENSION: u32 = 32767;

/// DOMMatrix represents a 2D transformation matrix.
///
/// The matrix is represented as:
/// ```text
/// | a c e |
/// | b d f |
/// | 0 0 1 |
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DOMMatrix {
    /// Scale X component.
    pub a: f32,
    /// Skew Y component.
    pub b: f32,
    /// Skew X component.
    pub c: f32,
    /// Scale Y component.
    pub d: f32,
    /// Translate X component.
    pub e: f32,
    /// Translate Y component.
    pub f: f32,
}

impl DOMMatrix {
    /// Create a new DOMMatrix with the specified components.
    pub fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Create an identity matrix.
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }
}

impl From<tiny_skia::Transform> for DOMMatrix {
    fn from(t: tiny_skia::Transform) -> Self {
        DOMMatrix {
            a: t.sx,
            b: t.ky,
            c: t.kx,
            d: t.sy,
            e: t.tx,
            f: t.ty,
        }
    }
}

impl From<DOMMatrix> for tiny_skia::Transform {
    fn from(m: DOMMatrix) -> Self {
        tiny_skia::Transform::from_row(m.a, m.b, m.c, m.d, m.e, m.f)
    }
}

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
            letter_spacing: 0.0,
            image_smoothing_enabled: true,
            image_smoothing_quality: ImageSmoothingQuality::default(),
        }
    }
}

/// Builder for Canvas2dContext.
pub struct Canvas2dContextBuilder {
    width: u32,
    height: u32,
    font_db: Option<fontdb::Database>,
}

impl Canvas2dContextBuilder {
    /// Create a new builder with specified dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            font_db: None,
        }
    }

    /// Set a custom font database (to share with other components).
    pub fn with_font_db(mut self, db: fontdb::Database) -> Self {
        self.font_db = Some(db);
        self
    }

    /// Build the Canvas2dContext.
    pub fn build(self) -> Canvas2dResult<Canvas2dContext> {
        Canvas2dContext::new_internal(self.width, self.height, self.font_db)
    }
}

/// Canvas 2D rendering context.
pub struct Canvas2dContext {
    /// Width of the canvas in pixels.
    width: u32,
    /// Height of the canvas in pixels.
    height: u32,
    /// Pixel buffer.
    pixmap: Pixmap,
    /// Font system for text rendering.
    font_system: FontSystem,
    /// Swash cache for glyph rasterization.
    swash_cache: SwashCache,
    /// Current drawing state.
    state: DrawingState,
    /// Stack of saved drawing states.
    state_stack: Vec<DrawingState>,
    /// Current path builder.
    path_builder: tiny_skia::PathBuilder,
    /// Current path position (for tracking subpath start).
    current_x: f32,
    current_y: f32,
    /// Subpath start position (for closePath).
    subpath_start_x: f32,
    subpath_start_y: f32,
}

impl Canvas2dContext {
    /// Create a new Canvas2dContext with the specified dimensions.
    pub fn new(width: u32, height: u32) -> Canvas2dResult<Self> {
        Self::new_internal(width, height, None)
    }

    /// Create a new builder for more configuration options.
    pub fn builder(width: u32, height: u32) -> Canvas2dContextBuilder {
        Canvas2dContextBuilder::new(width, height)
    }

    fn new_internal(
        width: u32,
        height: u32,
        font_db: Option<fontdb::Database>,
    ) -> Canvas2dResult<Self> {
        // Validate dimensions
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(Canvas2dError::InvalidDimensions { width, height });
        }

        // Create pixmap
        let pixmap =
            Pixmap::new(width, height).ok_or(Canvas2dError::InvalidDimensions { width, height })?;

        // Create font system
        let font_system = if let Some(db) = font_db {
            FontSystem::new_with_locale_and_db("en".to_string(), db)
        } else {
            FontSystem::new()
        };

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
            path_builder: tiny_skia::PathBuilder::new(),
            current_x: 0.0,
            current_y: 0.0,
            subpath_start_x: 0.0,
            subpath_start_y: 0.0,
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
        self.state_stack.push(self.state.clone());
    }

    /// Restore the previously saved drawing state.
    pub fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.state = state;
        }
    }

    // --- Style setters ---

    /// Set the fill style from a CSS color string.
    pub fn set_fill_style(&mut self, style: &str) -> Canvas2dResult<()> {
        let color = parse_color(style)?;
        self.state.fill_style = FillStyle::Color(color);
        Ok(())
    }

    /// Set the fill style from a tiny_skia Color.
    pub fn set_fill_style_color(&mut self, color: tiny_skia::Color) {
        self.state.fill_style = FillStyle::Color(color);
    }

    /// Set the stroke style from a CSS color string.
    pub fn set_stroke_style(&mut self, style: &str) -> Canvas2dResult<()> {
        let color = parse_color(style)?;
        self.state.stroke_style = FillStyle::Color(color);
        Ok(())
    }

    /// Set the stroke style from a tiny_skia Color.
    pub fn set_stroke_style_color(&mut self, color: tiny_skia::Color) {
        self.state.stroke_style = FillStyle::Color(color);
    }

    /// Set the line width.
    pub fn set_line_width(&mut self, width: f32) {
        self.state.line_width = width.max(0.0);
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
    pub fn set_miter_limit(&mut self, limit: f32) {
        self.state.miter_limit = limit.max(0.0);
    }

    /// Set the global alpha (opacity).
    pub fn set_global_alpha(&mut self, alpha: f32) {
        self.state.global_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Set the global composite operation (blend mode).
    pub fn set_global_composite_operation(&mut self, op: &str) {
        self.state.global_composite_operation = match op {
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
            _ => tiny_skia::BlendMode::SourceOver,
        };
    }

    /// Set the line dash pattern.
    pub fn set_line_dash(&mut self, segments: Vec<f32>) {
        self.state.line_dash = segments;
    }

    /// Get the current line dash pattern.
    pub fn get_line_dash(&self) -> &[f32] {
        &self.state.line_dash
    }

    /// Set the line dash offset.
    pub fn set_line_dash_offset(&mut self, offset: f32) {
        self.state.line_dash_offset = offset;
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
    fn get_image_filter_quality(&self) -> tiny_skia::FilterQuality {
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
    pub fn create_radial_gradient(
        &self,
        x0: f32,
        y0: f32,
        r0: f32,
        x1: f32,
        y1: f32,
        r1: f32,
    ) -> CanvasGradient {
        CanvasGradient::new_radial(x0, y0, r0, x1, y1, r1)
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

    /// Create a pattern from an existing canvas (pixmap reference).
    pub fn create_pattern_from_canvas(
        &self,
        pixmap: tiny_skia::PixmapRef,
        repetition: &str,
    ) -> Canvas2dResult<Arc<CanvasPattern>> {
        let rep = repetition.parse::<Repetition>()?;
        let pattern = CanvasPattern::from_pixmap_ref(pixmap, rep)?;
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

    // --- Font and text ---

    /// Set the font from a CSS font string.
    pub fn set_font(&mut self, font: &str) -> Canvas2dResult<()> {
        self.state.font = parse_font(font)?;
        Ok(())
    }

    /// Set the text alignment.
    pub fn set_text_align(&mut self, align: TextAlign) {
        self.state.text_align = align;
    }

    /// Set the text baseline.
    pub fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.state.text_baseline = baseline;
    }

    /// Set the letter spacing for text rendering (in pixels).
    pub fn set_letter_spacing(&mut self, spacing: f32) {
        self.state.letter_spacing = spacing;
    }

    /// Get the current letter spacing (in pixels).
    pub fn get_letter_spacing(&self) -> f32 {
        self.state.letter_spacing
    }

    /// Measure text and return metrics.
    pub fn measure_text(&mut self, text: &str) -> Canvas2dResult<TextMetrics> {
        crate::text::measure_text(&mut self.font_system, text, &self.state.font)
    }

    /// Fill text at the specified position.
    pub fn fill_text(&mut self, text: &str, x: f32, y: f32) {
        self.render_text_impl(text, x, y, None, true);
    }

    /// Fill text at the specified position with a maximum width.
    ///
    /// If the text width exceeds max_width, the text is horizontally scaled to fit.
    /// If max_width is <= 0, NaN, or the text would be scaled below 0.1%, nothing is rendered.
    pub fn fill_text_max_width(&mut self, text: &str, x: f32, y: f32, max_width: f32) {
        self.render_text_impl(text, x, y, Some(max_width), true);
    }

    /// Stroke text at the specified position.
    pub fn stroke_text(&mut self, text: &str, x: f32, y: f32) {
        self.render_text_impl(text, x, y, None, false);
    }

    /// Stroke text at the specified position with a maximum width.
    ///
    /// If the text width exceeds max_width, the text is horizontally scaled to fit.
    /// If max_width is <= 0, NaN, or the text would be scaled below 0.1%, nothing is rendered.
    pub fn stroke_text_max_width(&mut self, text: &str, x: f32, y: f32, max_width: f32) {
        self.render_text_impl(text, x, y, Some(max_width), false);
    }

    /// Internal text rendering using vector glyph paths (used by fillText and strokeText).
    fn render_text_impl(&mut self, text: &str, x: f32, y: f32, max_width: Option<f32>, fill: bool) {
        // Handle max_width edge cases: if <= 0 or NaN, don't render
        if let Some(mw) = max_width {
            if mw <= 0.0 || mw.is_nan() {
                return;
            }
        }

        let font = &self.state.font;
        let metrics = Metrics::new(font.size_px, font.size_px * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        // Build attributes from parsed font
        let family = font
            .families
            .first()
            .map(|f| Family::Name(f))
            .unwrap_or(Family::SansSerif);

        // Build attributes including letter spacing if set
        let letter_spacing = self.state.letter_spacing;
        let attrs = Attrs::new()
            .family(family)
            .weight(font.weight)
            .style(font.style)
            .letter_spacing(letter_spacing);

        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Get text dimensions for alignment
        let mut text_width: f32 = 0.0;
        for run in buffer.layout_runs() {
            text_width = text_width.max(run.line_w);
        }

        // Calculate horizontal scale factor for maxWidth
        let scale_x = if let Some(mw) = max_width {
            if mw.is_infinite() || text_width <= mw {
                // Infinity or text fits: no scaling needed
                1.0
            } else {
                // Text is too wide: calculate scale factor
                let scale = mw / text_width;
                // Don't render if scale would be too small (< 0.1%)
                if scale < 0.001 {
                    return;
                }
                scale
            }
        } else {
            1.0
        };

        // Calculate alignment offset (using scaled text width for alignment when maxWidth applies)
        let scaled_text_width = text_width * scale_x;
        let x_offset =
            crate::text::calculate_text_x_offset(scaled_text_width, self.state.text_align);

        // Calculate baseline offset
        let y_offset = crate::text::calculate_text_y_offset(font.size_px, self.state.text_baseline);

        // Get the paint for rendering text
        let style = if fill {
            self.state.fill_style.clone()
        } else {
            self.state.stroke_style.clone()
        };
        let Some(paint) = self.create_paint_from_style(&style) else {
            return; // Could not create paint
        };

        // Calculate base position with alignment offsets
        // Note: We use (x, y) as the anchor point, x_offset adjusts for alignment
        let base_x = x + x_offset;
        let base_y = y + y_offset;

        // Get the current transform
        let transform = self.state.transform;

        // For maxWidth scaling, we need to scale around the text anchor point (x position).
        // Build a combined transform that:
        // 1. Translates to put the anchor at origin
        // 2. Scales horizontally
        // 3. Translates back
        // 4. Applies global transform
        let scale_transform = if scale_x != 1.0 {
            // Scale around the x anchor point (keeping y unchanged)
            Transform::from_translate(x, 0.0)
                .pre_scale(scale_x, 1.0)
                .pre_translate(-x, 0.0)
                .post_concat(transform)
        } else {
            transform
        };

        // Render each glyph as a vector path
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((base_x, base_y), 1.0);

                // Get outline commands for this glyph
                if let Some(commands) = self
                    .swash_cache
                    .get_outline_commands(&mut self.font_system, physical_glyph.cache_key)
                {
                    // Build a path from the outline commands
                    // Note: Font outlines have Y pointing up, screen has Y pointing down
                    // so we negate Y coordinates during path building
                    let mut path_builder = tiny_skia::PathBuilder::new();
                    for cmd in commands {
                        match cmd {
                            Command::MoveTo(p) => path_builder.move_to(p.x, -p.y),
                            Command::LineTo(p) => path_builder.line_to(p.x, -p.y),
                            Command::QuadTo(ctrl, end) => {
                                path_builder.quad_to(ctrl.x, -ctrl.y, end.x, -end.y)
                            }
                            Command::CurveTo(c1, c2, end) => {
                                path_builder.cubic_to(c1.x, -c1.y, c2.x, -c2.y, end.x, -end.y)
                            }
                            Command::Close => path_builder.close(),
                        }
                    }

                    if let Some(path) = path_builder.finish() {
                        // Create a transform that positions the glyph correctly
                        // The outline commands are already scaled to pixel size
                        let glyph_transform = Transform::from_translate(
                            physical_glyph.x as f32,
                            physical_glyph.y as f32,
                        )
                        .post_concat(scale_transform);

                        // Fill the glyph path
                        self.pixmap.fill_path(
                            &path,
                            &paint,
                            tiny_skia::FillRule::Winding,
                            glyph_transform,
                            None,
                        );
                    }
                }
            }
        }
    }

    // --- Path operations ---

    /// Begin a new path.
    pub fn begin_path(&mut self) {
        self.path_builder = tiny_skia::PathBuilder::new();
    }

    /// Move to a point without drawing.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.path_builder.move_to(x, y);
        self.current_x = x;
        self.current_y = y;
        self.subpath_start_x = x;
        self.subpath_start_y = y;
    }

    /// Draw a line to a point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.path_builder.line_to(x, y);
        self.current_x = x;
        self.current_y = y;
    }

    /// Close the current subpath.
    pub fn close_path(&mut self) {
        self.path_builder.close();
        self.current_x = self.subpath_start_x;
        self.current_y = self.subpath_start_y;
    }

    /// Add a cubic bezier curve.
    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.path_builder.cubic_to(cp1x, cp1y, cp2x, cp2y, x, y);
        self.current_x = x;
        self.current_y = y;
    }

    /// Add a quadratic bezier curve.
    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.path_builder.quad_to(cpx, cpy, x, y);
        self.current_x = x;
        self.current_y = y;
    }

    /// Add a rectangle to the path.
    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        use crate::path::PathBuilderExt;
        self.path_builder.rect(x, y, width, height);
    }

    /// Add a rounded rectangle to the path with uniform corner radius.
    pub fn round_rect(&mut self, x: f32, y: f32, width: f32, height: f32, radius: f32) {
        self.round_rect_radii(x, y, width, height, [radius, radius, radius, radius]);
    }

    /// Add a rounded rectangle to the path with individual corner radii.
    ///
    /// The radii array specifies the corner radii in order:
    /// `[top-left, top-right, bottom-right, bottom-left]`
    pub fn round_rect_radii(&mut self, x: f32, y: f32, width: f32, height: f32, radii: [f32; 4]) {
        // Handle negative dimensions by adjusting position
        let (x, width) = if width < 0.0 {
            (x + width, -width)
        } else {
            (x, width)
        };
        let (y, height) = if height < 0.0 {
            (y + height, -height)
        } else {
            (y, height)
        };

        let [mut tl, mut tr, mut br, mut bl] = radii;

        // Clamp radii to non-negative
        tl = tl.max(0.0);
        tr = tr.max(0.0);
        br = br.max(0.0);
        bl = bl.max(0.0);

        // Scale radii if they exceed the rectangle dimensions
        // Per spec: scale all radii uniformly if they exceed dimensions
        let scale_x = width / (tl.max(bl) + tr.max(br)).max(1e-10);
        let scale_y = height / (tl.max(tr) + bl.max(br)).max(1e-10);
        let scale = scale_x.min(scale_y).min(1.0);

        if scale < 1.0 {
            tl *= scale;
            tr *= scale;
            br *= scale;
            bl *= scale;
        }

        // Build the rounded rectangle path using quadratic curves for corners
        // Start at the top edge, after the top-left corner
        self.path_builder.move_to(x + tl, y);

        // Top edge to top-right corner
        self.path_builder.line_to(x + width - tr, y);

        // Top-right corner
        if tr > 0.0 {
            self.path_builder.quad_to(x + width, y, x + width, y + tr);
        }

        // Right edge to bottom-right corner
        self.path_builder.line_to(x + width, y + height - br);

        // Bottom-right corner
        if br > 0.0 {
            self.path_builder
                .quad_to(x + width, y + height, x + width - br, y + height);
        }

        // Bottom edge to bottom-left corner
        self.path_builder.line_to(x + bl, y + height);

        // Bottom-left corner
        if bl > 0.0 {
            self.path_builder.quad_to(x, y + height, x, y + height - bl);
        }

        // Left edge to top-left corner
        self.path_builder.line_to(x, y + tl);

        // Top-left corner
        if tl > 0.0 {
            self.path_builder.quad_to(x, y, x + tl, y);
        }

        self.path_builder.close();
    }

    /// Add an arc to the path.
    pub fn arc(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        anticlockwise: bool,
    ) {
        crate::arc::arc(
            &mut self.path_builder,
            x,
            y,
            radius,
            start_angle,
            end_angle,
            anticlockwise,
        );
    }

    /// Add an arcTo segment to the path.
    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        crate::arc::arc_to(
            &mut self.path_builder,
            self.current_x,
            self.current_y,
            x1,
            y1,
            x2,
            y2,
            radius,
        );
    }

    /// Add an ellipse to the path.
    #[allow(clippy::too_many_arguments)]
    pub fn ellipse(
        &mut self,
        x: f32,
        y: f32,
        radius_x: f32,
        radius_y: f32,
        rotation: f32,
        start_angle: f32,
        end_angle: f32,
        anticlockwise: bool,
    ) {
        crate::arc::ellipse(
            &mut self.path_builder,
            x,
            y,
            radius_x,
            radius_y,
            rotation,
            start_angle,
            end_angle,
            anticlockwise,
        );
    }

    // --- Clipping ---

    /// Create a clipping region from the current path using the non-zero winding rule.
    pub fn clip(&mut self) {
        self.clip_with_rule(CanvasFillRule::NonZero);
    }

    /// Create a clipping region from the current path with the specified fill rule.
    pub fn clip_with_rule(&mut self, _fill_rule: CanvasFillRule) {
        // Note: The fill_rule is stored but used during mask creation in create_clip_mask()
        // For now, we store the path and use FillRule::Winding in the mask
        // A more complete implementation would store the fill rule with the clip path
        let path =
            std::mem::replace(&mut self.path_builder, tiny_skia::PathBuilder::new()).finish();

        if let Some(path) = path {
            self.state.clip_path = Some(path);
        }
    }

    // --- Drawing operations ---

    /// Fill the current path using the non-zero winding rule.
    pub fn fill(&mut self) {
        self.fill_with_rule(CanvasFillRule::NonZero);
    }

    /// Fill the current path with the specified fill rule.
    pub fn fill_with_rule(&mut self, fill_rule: CanvasFillRule) {
        let path =
            std::mem::replace(&mut self.path_builder, tiny_skia::PathBuilder::new()).finish();

        if let Some(path) = path {
            if let Some(paint) = self.create_fill_paint() {
                // Create clip mask if we have a clip path
                let clip_mask = self.create_clip_mask();
                self.pixmap.fill_path(
                    &path,
                    &paint,
                    fill_rule.into(),
                    self.state.transform,
                    clip_mask.as_ref(),
                );
            }
        }
    }

    /// Stroke the current path.
    pub fn stroke(&mut self) {
        let path =
            std::mem::replace(&mut self.path_builder, tiny_skia::PathBuilder::new()).finish();

        if let Some(path) = path {
            if let Some(paint) = self.create_stroke_paint() {
                let stroke = tiny_skia::Stroke {
                    width: self.state.line_width,
                    line_cap: self.state.line_cap.into(),
                    line_join: self.state.line_join.into(),
                    miter_limit: self.state.miter_limit,
                    dash: if self.state.line_dash.is_empty() {
                        None
                    } else {
                        tiny_skia::StrokeDash::new(
                            self.state.line_dash.clone(),
                            self.state.line_dash_offset,
                        )
                    },
                };

                let clip_mask = self.create_clip_mask();
                self.pixmap.stroke_path(
                    &path,
                    &paint,
                    &stroke,
                    self.state.transform,
                    clip_mask.as_ref(),
                );
            }
        }
    }

    // --- Path2D operations ---

    /// Fill a Path2D object using the non-zero winding rule.
    pub fn fill_path2d(&mut self, path: &mut Path2D) {
        self.fill_path2d_with_rule(path, CanvasFillRule::NonZero);
    }

    /// Fill a Path2D object with the specified fill rule.
    pub fn fill_path2d_with_rule(&mut self, path: &mut Path2D, fill_rule: CanvasFillRule) {
        if let Some(p) = path.get_path() {
            if let Some(paint) = self.create_fill_paint() {
                let clip_mask = self.create_clip_mask();
                self.pixmap.fill_path(
                    p,
                    &paint,
                    fill_rule.into(),
                    self.state.transform,
                    clip_mask.as_ref(),
                );
            }
        }
    }

    /// Stroke a Path2D object.
    pub fn stroke_path2d(&mut self, path: &mut Path2D) {
        if let Some(p) = path.get_path() {
            if let Some(paint) = self.create_stroke_paint() {
                let stroke = tiny_skia::Stroke {
                    width: self.state.line_width,
                    line_cap: self.state.line_cap.into(),
                    line_join: self.state.line_join.into(),
                    miter_limit: self.state.miter_limit,
                    dash: if self.state.line_dash.is_empty() {
                        None
                    } else {
                        tiny_skia::StrokeDash::new(
                            self.state.line_dash.clone(),
                            self.state.line_dash_offset,
                        )
                    },
                };

                let clip_mask = self.create_clip_mask();
                self.pixmap.stroke_path(
                    p,
                    &paint,
                    &stroke,
                    self.state.transform,
                    clip_mask.as_ref(),
                );
            }
        }
    }

    /// Clip to a Path2D object using the non-zero winding rule.
    pub fn clip_path2d(&mut self, path: &mut Path2D) {
        self.clip_path2d_with_rule(path, CanvasFillRule::NonZero);
    }

    /// Clip to a Path2D object with the specified fill rule.
    pub fn clip_path2d_with_rule(&mut self, path: &mut Path2D, _fill_rule: CanvasFillRule) {
        if let Some(p) = path.get_path() {
            self.state.clip_path = Some(p.clone());
        }
    }

    /// Fill a rectangle.
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
            if let Some(paint) = self.create_fill_paint() {
                let clip_mask = self.create_clip_mask();
                self.pixmap
                    .fill_rect(rect, &paint, self.state.transform, clip_mask.as_ref());
            }
        }
    }

    /// Stroke a rectangle.
    pub fn stroke_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.begin_path();
        self.rect(x, y, width, height);
        self.stroke();
    }

    /// Clear a rectangle (set pixels to transparent).
    pub fn clear_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
            let paint = tiny_skia::Paint {
                blend_mode: tiny_skia::BlendMode::Clear,
                ..Default::default()
            };
            let clip_mask = self.create_clip_mask();
            self.pixmap
                .fill_rect(rect, &paint, self.state.transform, clip_mask.as_ref());
        }
    }

    // --- Image drawing ---

    /// Draw an image at the specified position.
    ///
    /// This is the simplest form of drawImage - it draws the entire image
    /// at the specified (dx, dy) coordinates.
    pub fn draw_image(&mut self, image: tiny_skia::PixmapRef, dx: f32, dy: f32) {
        let paint = tiny_skia::PixmapPaint {
            opacity: self.state.global_alpha,
            blend_mode: self.state.global_composite_operation,
            quality: self.get_image_filter_quality(),
        };

        // Translate to destination position
        let transform = self.state.transform.pre_translate(dx, dy);

        let clip_mask = self.create_clip_mask();
        self.pixmap
            .draw_pixmap(0, 0, image, &paint, transform, clip_mask.as_ref());
    }

    /// Draw an image scaled to the specified dimensions.
    ///
    /// This form draws the entire source image scaled to fit within
    /// the destination rectangle (dx, dy, dw, dh).
    pub fn draw_image_scaled(
        &mut self,
        image: tiny_skia::PixmapRef,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        let paint = tiny_skia::PixmapPaint {
            opacity: self.state.global_alpha,
            blend_mode: self.state.global_composite_operation,
            quality: self.get_image_filter_quality(),
        };

        // Calculate scale factors
        let scale_x = dw / image.width() as f32;
        let scale_y = dh / image.height() as f32;

        // Translate to destination position, then scale
        let transform = self
            .state
            .transform
            .pre_translate(dx, dy)
            .pre_scale(scale_x, scale_y);

        let clip_mask = self.create_clip_mask();
        self.pixmap
            .draw_pixmap(0, 0, image, &paint, transform, clip_mask.as_ref());
    }

    /// Draw a portion of an image to a destination rectangle.
    ///
    /// This form extracts a source rectangle (sx, sy, sw, sh) from the image
    /// and draws it into the destination rectangle (dx, dy, dw, dh).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_image_cropped(
        &mut self,
        image: tiny_skia::PixmapRef,
        sx: f32,
        sy: f32,
        sw: f32,
        sh: f32,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        // Clamp source rectangle to image bounds
        let sx = sx.max(0.0);
        let sy = sy.max(0.0);
        let sw = sw.min(image.width() as f32 - sx);
        let sh = sh.min(image.height() as f32 - sy);

        if sw <= 0.0 || sh <= 0.0 || dw <= 0.0 || dh <= 0.0 {
            return;
        }

        // Create a sub-image by creating a temporary pixmap with just the source region
        let sub_width = sw.ceil() as u32;
        let sub_height = sh.ceil() as u32;

        if let Some(mut sub_pixmap) = tiny_skia::Pixmap::new(sub_width, sub_height) {
            // Copy the source region to the sub-pixmap
            let src_x = sx.floor() as i32;
            let src_y = sy.floor() as i32;

            // Draw the source image offset to extract the region
            let extract_paint = tiny_skia::PixmapPaint::default();
            let extract_transform = Transform::from_translate(-src_x as f32, -src_y as f32);
            sub_pixmap.draw_pixmap(0, 0, image, &extract_paint, extract_transform, None);

            // Now draw the extracted region scaled to the destination
            self.draw_image_scaled(sub_pixmap.as_ref(), dx, dy, dw, dh);
        }
    }

    // --- Transform operations ---

    /// Translate the canvas.
    pub fn translate(&mut self, x: f32, y: f32) {
        self.state.transform = self.state.transform.pre_translate(x, y);
    }

    /// Rotate the canvas.
    pub fn rotate(&mut self, angle: f32) {
        let cos = angle.cos();
        let sin = angle.sin();
        let rotation = Transform::from_row(cos, sin, -sin, cos, 0.0, 0.0);
        self.state.transform = self.state.transform.pre_concat(rotation);
    }

    /// Scale the canvas.
    pub fn scale(&mut self, x: f32, y: f32) {
        self.state.transform = self.state.transform.pre_scale(x, y);
    }

    /// Apply a transform matrix.
    pub fn transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        let t = Transform::from_row(a, b, c, d, e, f);
        self.state.transform = self.state.transform.pre_concat(t);
    }

    /// Set the transform matrix (replacing the current one).
    pub fn set_transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        self.state.transform = Transform::from_row(a, b, c, d, e, f);
    }

    /// Reset the transform to identity.
    pub fn reset_transform(&mut self) {
        self.state.transform = Transform::identity();
    }

    /// Get the current transformation matrix.
    pub fn get_transform(&self) -> DOMMatrix {
        self.state.transform.into()
    }

    /// Set the transform from a DOMMatrix.
    pub fn set_transform_matrix(&mut self, matrix: DOMMatrix) {
        self.state.transform = matrix.into();
    }

    // --- Output ---

    /// Get image data for a region of the canvas.
    pub fn get_image_data(&self, x: i32, y: i32, width: u32, height: u32) -> Vec<u8> {
        let mut data = vec![0u8; (width * height * 4) as usize];

        for dy in 0..height {
            for dx in 0..width {
                let src_x = x + dx as i32;
                let src_y = y + dy as i32;

                let dst_idx = ((dy * width + dx) * 4) as usize;

                if src_x >= 0
                    && src_x < self.width as i32
                    && src_y >= 0
                    && src_y < self.height as i32
                {
                    let src_idx = (src_y as u32 * self.width + src_x as u32) as usize;
                    let pixel = self.pixmap.data()[src_idx * 4..src_idx * 4 + 4].to_vec();

                    // Convert from premultiplied alpha to straight alpha
                    let a = pixel[3];
                    if a == 0 {
                        data[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
                    } else if a == 255 {
                        data[dst_idx..dst_idx + 4].copy_from_slice(&pixel);
                    } else {
                        let alpha_f = a as f32 / 255.0;
                        data[dst_idx] = (pixel[0] as f32 / alpha_f).min(255.0) as u8;
                        data[dst_idx + 1] = (pixel[1] as f32 / alpha_f).min(255.0) as u8;
                        data[dst_idx + 2] = (pixel[2] as f32 / alpha_f).min(255.0) as u8;
                        data[dst_idx + 3] = a;
                    }
                }
            }
        }

        data
    }

    /// Write image data to the canvas at the specified position.
    ///
    /// The data must be in non-premultiplied RGBA format (standard ImageData format).
    /// This bypasses compositing operations and writes pixels directly.
    ///
    /// # Arguments
    /// * `data` - RGBA pixel data (4 bytes per pixel, non-premultiplied alpha)
    /// * `width` - Width of the image data
    /// * `height` - Height of the image data
    /// * `dx` - Destination x coordinate
    /// * `dy` - Destination y coordinate
    pub fn put_image_data(&mut self, data: &[u8], width: u32, height: u32, dx: i32, dy: i32) {
        self.put_image_data_dirty(
            data,
            width,
            height,
            dx,
            dy,
            0,
            0,
            width as i32,
            height as i32,
        );
    }

    /// Write a portion of image data to the canvas.
    ///
    /// The dirty rectangle specifies which portion of the source data to write.
    /// Pixels outside the canvas bounds are silently ignored.
    ///
    /// # Arguments
    /// * `data` - RGBA pixel data (4 bytes per pixel, non-premultiplied alpha)
    /// * `width` - Width of the image data
    /// * `height` - Height of the image data
    /// * `dx` - Destination x coordinate
    /// * `dy` - Destination y coordinate
    /// * `dirty_x` - X offset into the source data
    /// * `dirty_y` - Y offset into the source data
    /// * `dirty_width` - Width of region to copy
    /// * `dirty_height` - Height of region to copy
    #[allow(clippy::too_many_arguments)]
    pub fn put_image_data_dirty(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        dx: i32,
        dy: i32,
        dirty_x: i32,
        dirty_y: i32,
        dirty_width: i32,
        dirty_height: i32,
    ) {
        // Clamp dirty rect to source image bounds
        let dirty_x = dirty_x.max(0).min(width as i32);
        let dirty_y = dirty_y.max(0).min(height as i32);
        let dirty_width = dirty_width.max(0).min(width as i32 - dirty_x);
        let dirty_height = dirty_height.max(0).min(height as i32 - dirty_y);

        if dirty_width <= 0 || dirty_height <= 0 {
            return; // Nothing to draw
        }

        // Calculate destination coordinates for the dirty region
        let dest_x = dx + dirty_x;
        let dest_y = dy + dirty_y;

        // Get mutable access to pixmap data
        let canvas_width = self.width as i32;
        let canvas_height = self.height as i32;
        let pixmap_data = self.pixmap.data_mut();

        for sy in 0..dirty_height {
            let src_row = dirty_y + sy;
            let dst_row = dest_y + sy;

            // Skip if destination row is out of bounds
            if dst_row < 0 || dst_row >= canvas_height {
                continue;
            }

            for sx in 0..dirty_width {
                let src_col = dirty_x + sx;
                let dst_col = dest_x + sx;

                // Skip if destination column is out of bounds
                if dst_col < 0 || dst_col >= canvas_width {
                    continue;
                }

                // Calculate source and destination indices
                let src_idx = ((src_row as u32 * width + src_col as u32) * 4) as usize;
                let dst_idx = ((dst_row as u32 * self.width + dst_col as u32) * 4) as usize;

                // Read source pixel (non-premultiplied RGBA)
                let r = data[src_idx];
                let g = data[src_idx + 1];
                let b = data[src_idx + 2];
                let a = data[src_idx + 3];

                // Convert to premultiplied alpha using integer math
                // Formula: (color * alpha + 127) / 255 for proper rounding
                let (pr, pg, pb) = if a == 255 {
                    (r, g, b) // No conversion needed for fully opaque
                } else if a == 0 {
                    (0, 0, 0) // Fully transparent
                } else {
                    let a16 = a as u16;
                    (
                        ((r as u16 * a16 + 127) / 255) as u8,
                        ((g as u16 * a16 + 127) / 255) as u8,
                        ((b as u16 * a16 + 127) / 255) as u8,
                    )
                };

                // Write to destination (bypasses compositing - direct pixel write)
                pixmap_data[dst_idx] = pr;
                pixmap_data[dst_idx + 1] = pg;
                pixmap_data[dst_idx + 2] = pb;
                pixmap_data[dst_idx + 3] = a;
            }
        }
    }

    /// Export the canvas as PNG data.
    pub fn to_png(&self) -> Canvas2dResult<Vec<u8>> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);

            let mut writer = encoder.write_header()?;

            // Convert from premultiplied to straight alpha for PNG
            let data = self.get_image_data(0, 0, self.width, self.height);
            writer.write_image_data(&data)?;
        }
        Ok(buf)
    }

    /// Get a reference to the underlying pixmap.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    /// Get a mutable reference to the underlying pixmap.
    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.pixmap
    }

    // --- Private helpers ---

    fn create_clip_mask(&self) -> Option<tiny_skia::Mask> {
        self.state.clip_path.as_ref().and_then(|clip_path| {
            let mut mask = tiny_skia::Mask::new(self.width, self.height)?;
            mask.fill_path(
                clip_path,
                tiny_skia::FillRule::Winding,
                true,
                self.state.transform,
            );
            Some(mask)
        })
    }

    fn create_fill_paint(&self) -> Option<tiny_skia::Paint<'static>> {
        self.create_paint_from_style(&self.state.fill_style)
    }

    fn create_stroke_paint(&self) -> Option<tiny_skia::Paint<'static>> {
        self.create_paint_from_style(&self.state.stroke_style)
    }

    fn create_paint_from_style(&self, style: &FillStyle) -> Option<tiny_skia::Paint<'static>> {
        let mut paint = tiny_skia::Paint {
            anti_alias: true,
            blend_mode: self.state.global_composite_operation,
            ..Default::default()
        };

        match style {
            FillStyle::Color(color) => {
                let mut color = *color;
                // Apply global alpha
                if self.state.global_alpha < 1.0 {
                    color.set_alpha((color.alpha() * self.state.global_alpha).clamp(0.0, 1.0));
                }
                paint.set_color(color);
                Some(paint)
            }
            FillStyle::LinearGradient(gradient) | FillStyle::RadialGradient(gradient) => {
                let shader = self.create_gradient_shader(gradient)?;
                paint.shader = shader;
                Some(paint)
            }
            FillStyle::Pattern(pattern) => {
                let shader = pattern.create_shader(
                    self.pixmap.width(),
                    self.pixmap.height(),
                    self.state.transform,
                )?;
                paint.shader = shader;
                Some(paint)
            }
        }
    }

    fn create_gradient_shader(
        &self,
        gradient: &CanvasGradient,
    ) -> Option<tiny_skia::Shader<'static>> {
        if gradient.stops.is_empty() {
            return None;
        }

        let stops: Vec<tiny_skia::GradientStop> = gradient
            .stops
            .iter()
            .map(|stop| {
                let mut color = stop.color;
                if self.state.global_alpha < 1.0 {
                    color.set_alpha((color.alpha() * self.state.global_alpha).clamp(0.0, 1.0));
                }
                tiny_skia::GradientStop::new(stop.offset as f32, color)
            })
            .collect();

        match &gradient.gradient_type {
            GradientType::Linear { x0, y0, x1, y1 } => tiny_skia::LinearGradient::new(
                tiny_skia::Point { x: *x0, y: *y0 },
                tiny_skia::Point { x: *x1, y: *y1 },
                stops,
                tiny_skia::SpreadMode::Pad,
                self.state.transform,
            ),
            GradientType::Radial {
                x0,
                y0,
                r0: _,
                x1,
                y1,
                r1,
            } => {
                // tiny_skia's RadialGradient::new(start, end, radius, ...)
                // - start: where gradient originates (inner circle center)
                // - end: outer circle center
                // - radius: outer circle radius
                // Note: r0 (inner radius) is not directly supported by tiny_skia
                tiny_skia::RadialGradient::new(
                    tiny_skia::Point { x: *x0, y: *y0 },
                    tiny_skia::Point { x: *x1, y: *y1 },
                    *r1,
                    stops,
                    tiny_skia::SpreadMode::Pad,
                    self.state.transform,
                )
            }
        }
    }
}

/// Parse a CSS color string into a tiny_skia::Color.
fn parse_color(s: &str) -> Canvas2dResult<tiny_skia::Color> {
    let parsed = csscolorparser::parse(s)
        .map_err(|e| Canvas2dError::ColorParseError(format!("{}: {}", s, e)))?;

    let [r, g, b, a] = parsed.to_array();
    Ok(tiny_skia::Color::from_rgba(r, g, b, a).unwrap_or(tiny_skia::Color::BLACK))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context() {
        let ctx = Canvas2dContext::new(100, 100);
        assert!(ctx.is_ok());
    }

    #[test]
    fn test_invalid_dimensions() {
        let ctx = Canvas2dContext::new(0, 100);
        assert!(matches!(ctx, Err(Canvas2dError::InvalidDimensions { .. })));
    }

    #[test]
    fn test_save_restore() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_fill_style("#ff0000").unwrap();
        ctx.save();
        ctx.set_fill_style("#00ff00").unwrap();
        ctx.restore();
        // State should be restored
    }

    #[test]
    fn test_fill_rect() {
        let mut ctx = Canvas2dContext::new(100, 100).unwrap();
        ctx.set_fill_style("#ff0000").unwrap();
        ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
        // Verify the pixmap has non-zero data
        assert!(ctx.pixmap().data().iter().any(|&b| b != 0));
    }
}
