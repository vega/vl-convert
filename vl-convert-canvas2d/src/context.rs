//! Canvas 2D rendering context implementation.

use crate::error::{Canvas2dError, Canvas2dResult};
use crate::font_config::{font_config_to_fontdb, FontConfig, ResolvedFontConfig};
use crate::font_parser::{parse_font, ParsedFont};
use crate::geometry::{
    ArcParams, ArcToParams, CanvasColor, CanvasImageDataRef, CubicBezierParams, DirtyRect,
    EllipseParams, ImageCropParams, QuadraticBezierParams, RadialGradientParams, RectParams,
    RoundRectParams,
};
use crate::gradient::{CanvasGradient, GradientType};
use crate::path2d::Path2D;
use crate::pattern::{CanvasPattern, Repetition};
use crate::style::{
    CanvasFillRule, FillStyle, FontStretch, ImageSmoothingQuality, LineCap, LineJoin, TextAlign,
    TextBaseline,
};
use crate::text::TextMetrics;
use cosmic_text::{
    Attrs, Buffer, CacheKeyFlags, Command, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use std::collections::HashMap;
use std::sync::Arc;
use tiny_skia::{PathSegment, Pixmap, Transform};

/// Maximum canvas dimension (same as Chrome).
const MAX_DIMENSION: u32 = 32767;

/// Maximum number of bytes retained by the per-context pattern pixmap cache.
const PATTERN_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PatternCacheKey {
    pattern_id: u64,
    repetition: Repetition,
    /// Cache dimensions (0,0 sentinel for Repeat mode).
    canvas_width: u32,
    canvas_height: u32,
}

#[derive(Debug)]
struct PatternCacheEntry {
    pixmap: Arc<Pixmap>,
    size_bytes: usize,
    last_used: u64,
}

#[derive(Debug)]
struct PatternPixmapCache {
    max_bytes: usize,
    total_bytes: usize,
    clock: u64,
    entries: HashMap<PatternCacheKey, PatternCacheEntry>,
}

impl PatternPixmapCache {
    fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            total_bytes: 0,
            clock: 0,
            entries: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
        self.clock = 0;
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn next_tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    fn get_or_insert(
        &mut self,
        key: PatternCacheKey,
        create: impl FnOnce() -> Option<Pixmap>,
    ) -> Option<Arc<Pixmap>> {
        if self.entries.contains_key(&key) {
            let tick = self.next_tick();
            let entry = self.entries.get_mut(&key)?;
            entry.last_used = tick;
            return Some(Arc::clone(&entry.pixmap));
        }

        let pixmap = create()?;
        let size_bytes = pixmap.data().len();
        let pixmap = Arc::new(pixmap);

        // Avoid pinning a single oversize pixmap in cache.
        if size_bytes > self.max_bytes {
            return Some(pixmap);
        }

        let tick = self.next_tick();
        self.total_bytes += size_bytes;
        self.entries.insert(
            key,
            PatternCacheEntry {
                pixmap: Arc::clone(&pixmap),
                size_bytes,
                last_used: tick,
            },
        );

        self.evict_to_budget();

        Some(pixmap)
    }

    fn evict_to_budget(&mut self) {
        while self.total_bytes > self.max_bytes {
            let lru_key = self
                .entries
                .iter()
                .min_by_key(|(_key, entry)| entry.last_used)
                .map(|(key, _entry)| *key);

            let Some(key) = lru_key else {
                break;
            };

            if let Some(entry) = self.entries.remove(&key) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.size_bytes);
            } else {
                break;
            }
        }
    }
}

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
    /// Fill rule associated with the current clipping path.
    clip_fill_rule: CanvasFillRule,
    /// Stack of saved clip fill rules (parallel to state_stack).
    clip_fill_rule_stack: Vec<CanvasFillRule>,
    /// Current path builder.
    path_builder: tiny_skia::PathBuilder,
    /// Current path position (for tracking subpath start).
    current_x: f32,
    current_y: f32,
    /// Subpath start position (for closePath).
    subpath_start_x: f32,
    subpath_start_y: f32,
    /// Whether the path has a current point (for arc/ellipse line_to vs move_to).
    has_current_point: bool,
    /// Owned cache of pattern backing pixmaps used for tiny-skia shader lifetimes.
    pattern_pixmap_cache: PatternPixmapCache,
}

impl Canvas2dContext {
    /// Create a new Canvas2dContext with the specified dimensions.
    ///
    /// Uses `FontConfig::default()` which loads system fonts and sets up
    /// standard generic family mappings (sans-serif, serif, monospace).
    pub fn new(width: u32, height: u32) -> Canvas2dResult<Self> {
        let db = font_config_to_fontdb(&FontConfig::default());
        Self::new_internal(width, height, db)
    }

    /// Create a new Canvas2dContext with the specified dimensions and font configuration.
    pub fn with_config(width: u32, height: u32, config: FontConfig) -> Canvas2dResult<Self> {
        let db = font_config_to_fontdb(&config);
        Self::new_internal(width, height, db)
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
        Self::new_internal(width, height, resolved.fontdb.clone())
    }

    fn new_internal(width: u32, height: u32, font_db: fontdb::Database) -> Canvas2dResult<Self> {
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

    /// Set the font stretch (width).
    pub fn set_font_stretch(&mut self, stretch: FontStretch) {
        self.state.font.stretch = stretch;
    }

    /// Get the current font stretch.
    pub fn get_font_stretch(&self) -> FontStretch {
        self.state.font.stretch
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
        log::debug!(target: "canvas", "fillText \"{}\" {} {}", text, x, y);
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
        log::debug!(target: "canvas", "strokeText \"{}\" {} {}", text, x, y);
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
        // Use get_family_with_fallback for proper font resolution (same as measure_text)
        let mut resolved_name: Option<String> = None;
        let resolution = font
            .families
            .first()
            .map(|f| {
                crate::text::get_family_with_fallback(&self.font_system, f, &mut resolved_name)
            })
            .unwrap_or(crate::text::FamilyResolution {
                family: Family::SansSerif,
                weight_override: None,
            });

        // Use weight from post_script_name match if available, otherwise use parsed CSS weight
        let weight = resolution.weight_override.unwrap_or(font.weight);

        // Build attributes including letter spacing if set
        // Disable hinting to match SVG text rendering (usvg doesn't apply hinting)
        let letter_spacing = self.state.letter_spacing;
        let attrs = Attrs::new()
            .family(resolution.family)
            .weight(weight)
            .style(font.style)
            .stretch(font.stretch.into())
            .letter_spacing(letter_spacing)
            .cache_key_flags(CacheKeyFlags::DISABLE_HINTING);

        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Get text dimensions for alignment
        let mut text_width: f32 = 0.0;
        let mut text_ascent: f32 = 0.0;
        let mut text_descent: f32 = 0.0;
        for run in buffer.layout_runs() {
            text_width = text_width.max(run.line_w);
            text_ascent = text_ascent.max(run.line_y - run.line_top);
            text_descent = text_descent.max((run.line_top + run.line_height) - run.line_y);
        }
        if text_ascent == 0.0 && text_descent == 0.0 {
            text_ascent = font.size_px * 0.8;
            text_descent = font.size_px * 0.2;
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

        // Calculate alignment offset using ORIGINAL text width.
        // The scale transform (applied around x) will handle making the scaled text
        // properly aligned - if we used scaled_text_width here, we'd double-adjust.
        let x_offset = crate::text::calculate_text_x_offset(text_width, self.state.text_align);

        // Calculate baseline offset
        let y_offset = crate::text::calculate_text_y_offset(
            text_ascent,
            text_descent,
            self.state.text_baseline,
        );

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

        // Get the paint for rendering text and render while it's alive.
        let style = if fill {
            self.state.fill_style.clone()
        } else {
            self.state.stroke_style.clone()
        };
        let _ = self.with_paint_from_style(style, |ctx, paint| {
            // Render each glyph as a vector path
            for run in buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    // Get the cache key for outline retrieval (physical() provides this)
                    let physical_glyph = glyph.physical((base_x, base_y), 1.0);

                    // Calculate floating-point glyph position for sub-pixel precision
                    // (matching how usvg/resvg positions glyphs)
                    let glyph_x = base_x + glyph.x + glyph.font_size * glyph.x_offset;
                    let glyph_y = base_y + glyph.y - glyph.font_size * glyph.y_offset;

                    // Get outline commands for this glyph
                    if let Some(commands) = ctx
                        .swash_cache
                        .get_outline_commands(&mut ctx.font_system, physical_glyph.cache_key)
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
                            // Using floating-point position for sub-pixel precision
                            let glyph_transform = Transform::from_translate(glyph_x, glyph_y)
                                .post_concat(scale_transform);

                            if fill {
                                // Fill the glyph path
                                ctx.pixmap.fill_path(
                                    &path,
                                    paint,
                                    tiny_skia::FillRule::Winding,
                                    glyph_transform,
                                    None,
                                );
                            } else {
                                // Stroke the glyph path
                                let stroke = tiny_skia::Stroke {
                                    width: ctx.state.line_width,
                                    line_cap: ctx.state.line_cap.into(),
                                    line_join: ctx.state.line_join.into(),
                                    miter_limit: ctx.state.miter_limit,
                                    dash: if ctx.state.line_dash.is_empty() {
                                        None
                                    } else {
                                        tiny_skia::StrokeDash::new(
                                            ctx.state.line_dash.clone(),
                                            ctx.state.line_dash_offset,
                                        )
                                    },
                                };
                                ctx.pixmap.stroke_path(
                                    &path,
                                    paint,
                                    &stroke,
                                    glyph_transform,
                                    None,
                                );
                            }
                        }
                    }
                }
            }
        });
    }

    // --- Path operations ---

    /// Begin a new path.
    pub fn begin_path(&mut self) {
        log::debug!(target: "canvas", "beginPath");
        self.path_builder = tiny_skia::PathBuilder::new();
        self.has_current_point = false;
    }

    /// Transform a point by the current transformation matrix.
    /// Canvas 2D spec requires path coordinates to be transformed when added to the path.
    fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        Self::map_point_with_transform(&self.state.transform, x, y)
    }

    fn map_point_with_transform(transform: &Transform, x: f32, y: f32) -> (f32, f32) {
        (
            transform.sx * x + transform.kx * y + transform.tx,
            transform.ky * x + transform.sy * y + transform.ty,
        )
    }

    fn append_transformed_path(
        &mut self,
        path: &tiny_skia::Path,
        transform: Transform,
        connect_first_move: bool,
        skip_first_move: bool,
    ) {
        let mut saw_first_move = false;

        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    let (x, y) = Self::map_point_with_transform(&transform, p.x, p.y);

                    if !saw_first_move {
                        saw_first_move = true;
                        if skip_first_move {
                            if !self.has_current_point {
                                self.path_builder.move_to(x, y);
                                self.subpath_start_x = x;
                                self.subpath_start_y = y;
                                self.current_x = x;
                                self.current_y = y;
                                self.has_current_point = true;
                            }
                            continue;
                        }

                        if connect_first_move && self.has_current_point {
                            self.path_builder.line_to(x, y);
                        } else {
                            self.path_builder.move_to(x, y);
                            self.subpath_start_x = x;
                            self.subpath_start_y = y;
                        }
                    } else {
                        self.path_builder.move_to(x, y);
                        self.subpath_start_x = x;
                        self.subpath_start_y = y;
                    }

                    self.current_x = x;
                    self.current_y = y;
                    self.has_current_point = true;
                }
                PathSegment::LineTo(p) => {
                    let (x, y) = Self::map_point_with_transform(&transform, p.x, p.y);
                    self.path_builder.line_to(x, y);
                    self.current_x = x;
                    self.current_y = y;
                    self.has_current_point = true;
                }
                PathSegment::QuadTo(ctrl, p) => {
                    let (cx, cy) = Self::map_point_with_transform(&transform, ctrl.x, ctrl.y);
                    let (x, y) = Self::map_point_with_transform(&transform, p.x, p.y);
                    self.path_builder.quad_to(cx, cy, x, y);
                    self.current_x = x;
                    self.current_y = y;
                    self.has_current_point = true;
                }
                PathSegment::CubicTo(ctrl1, ctrl2, p) => {
                    let (c1x, c1y) = Self::map_point_with_transform(&transform, ctrl1.x, ctrl1.y);
                    let (c2x, c2y) = Self::map_point_with_transform(&transform, ctrl2.x, ctrl2.y);
                    let (x, y) = Self::map_point_with_transform(&transform, p.x, p.y);
                    self.path_builder.cubic_to(c1x, c1y, c2x, c2y, x, y);
                    self.current_x = x;
                    self.current_y = y;
                    self.has_current_point = true;
                }
                PathSegment::Close => {
                    self.path_builder.close();
                    self.current_x = self.subpath_start_x;
                    self.current_y = self.subpath_start_y;
                    self.has_current_point = true;
                }
            }
        }
    }

    /// Move to a point without drawing.
    pub fn move_to(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "moveTo {} {}", x, y);
        let (tx, ty) = self.transform_point(x, y);
        self.path_builder.move_to(tx, ty);
        self.current_x = tx;
        self.current_y = ty;
        self.subpath_start_x = tx;
        self.subpath_start_y = ty;
        self.has_current_point = true;
    }

    /// Draw a line to a point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "lineTo {} {}", x, y);
        let (tx, ty) = self.transform_point(x, y);
        self.path_builder.line_to(tx, ty);
        self.current_x = tx;
        self.current_y = ty;
        self.has_current_point = true;
    }

    /// Close the current subpath.
    pub fn close_path(&mut self) {
        log::debug!(target: "canvas", "closePath");
        self.path_builder.close();
        self.current_x = self.subpath_start_x;
        self.current_y = self.subpath_start_y;
    }

    /// Add a cubic bezier curve.
    pub fn bezier_curve_to(&mut self, params: &CubicBezierParams) {
        let (tcp1x, tcp1y) = self.transform_point(params.cp1x, params.cp1y);
        let (tcp2x, tcp2y) = self.transform_point(params.cp2x, params.cp2y);
        let (tx, ty) = self.transform_point(params.x, params.y);
        self.path_builder
            .cubic_to(tcp1x, tcp1y, tcp2x, tcp2y, tx, ty);
        self.current_x = tx;
        self.current_y = ty;
        self.has_current_point = true;
    }

    /// Add a quadratic bezier curve.
    pub fn quadratic_curve_to(&mut self, params: &QuadraticBezierParams) {
        let (tcpx, tcpy) = self.transform_point(params.cpx, params.cpy);
        let (tx, ty) = self.transform_point(params.x, params.y);
        self.path_builder.quad_to(tcpx, tcpy, tx, ty);
        self.current_x = tx;
        self.current_y = ty;
        self.has_current_point = true;
    }

    /// Add a rectangle to the path.
    pub fn rect(&mut self, params: &RectParams) {
        log::debug!(target: "canvas", "rect {} {} {} {}", params.x, params.y, params.width, params.height);
        // Transform all four corners
        let (x0, y0) = self.transform_point(params.x, params.y);
        let (x1, y1) = self.transform_point(params.x + params.width, params.y);
        let (x2, y2) = self.transform_point(params.x + params.width, params.y + params.height);
        let (x3, y3) = self.transform_point(params.x, params.y + params.height);

        self.path_builder.move_to(x0, y0);
        self.path_builder.line_to(x1, y1);
        self.path_builder.line_to(x2, y2);
        self.path_builder.line_to(x3, y3);
        self.path_builder.close();

        self.current_x = x0;
        self.current_y = y0;
        self.subpath_start_x = x0;
        self.subpath_start_y = y0;
        self.has_current_point = true;
    }

    /// Add a rounded rectangle to the path.
    pub fn round_rect(&mut self, params: &RoundRectParams) {
        use crate::geometry::CornerRadius;

        // Handle negative dimensions by adjusting position
        let (x, width) = if params.width < 0.0 {
            (params.x + params.width, -params.width)
        } else {
            (params.x, params.width)
        };
        let (y, height) = if params.height < 0.0 {
            (params.y + params.height, -params.height)
        } else {
            (params.y, params.height)
        };

        let [mut tl, mut tr, mut br, mut bl] = params.radii;

        // Clamp radii to non-negative
        tl = CornerRadius {
            x: tl.x.max(0.0),
            y: tl.y.max(0.0),
        };
        tr = CornerRadius {
            x: tr.x.max(0.0),
            y: tr.y.max(0.0),
        };
        br = CornerRadius {
            x: br.x.max(0.0),
            y: br.y.max(0.0),
        };
        bl = CornerRadius {
            x: bl.x.max(0.0),
            y: bl.y.max(0.0),
        };

        // Scale radii uniformly if they exceed the rectangle dimensions
        // Top edge: tl.x + tr.x <= width
        // Bottom edge: bl.x + br.x <= width
        // Left edge: tl.y + bl.y <= height
        // Right edge: tr.y + br.y <= height
        let top = (tl.x + tr.x).max(1e-10);
        let bottom = (bl.x + br.x).max(1e-10);
        let left = (tl.y + bl.y).max(1e-10);
        let right = (tr.y + br.y).max(1e-10);
        let scale = (width / top)
            .min(width / bottom)
            .min(height / left)
            .min(height / right)
            .min(1.0);

        if scale < 1.0 {
            tl.x *= scale;
            tl.y *= scale;
            tr.x *= scale;
            tr.y *= scale;
            br.x *= scale;
            br.y *= scale;
            bl.x *= scale;
            bl.y *= scale;
        }

        // Kappa for quarter-ellipse cubic Bezier approximation
        const K: f32 = 0.552_284_8;

        // Build rounded rectangle path with elliptical corners
        self.path_builder.move_to(x + tl.x, y);

        // Top edge
        self.path_builder.line_to(x + width - tr.x, y);

        // Top-right corner
        if tr.x > 0.0 || tr.y > 0.0 {
            self.path_builder.cubic_to(
                x + width - tr.x + tr.x * K,
                y,
                x + width,
                y + tr.y - tr.y * K,
                x + width,
                y + tr.y,
            );
        }

        // Right edge
        self.path_builder.line_to(x + width, y + height - br.y);

        // Bottom-right corner
        if br.x > 0.0 || br.y > 0.0 {
            self.path_builder.cubic_to(
                x + width,
                y + height - br.y + br.y * K,
                x + width - br.x + br.x * K,
                y + height,
                x + width - br.x,
                y + height,
            );
        }

        // Bottom edge
        self.path_builder.line_to(x + bl.x, y + height);

        // Bottom-left corner
        if bl.x > 0.0 || bl.y > 0.0 {
            self.path_builder.cubic_to(
                x + bl.x - bl.x * K,
                y + height,
                x,
                y + height - bl.y + bl.y * K,
                x,
                y + height - bl.y,
            );
        }

        // Left edge
        self.path_builder.line_to(x, y + tl.y);

        // Top-left corner
        if tl.x > 0.0 || tl.y > 0.0 {
            self.path_builder
                .cubic_to(x, y + tl.y - tl.y * K, x + tl.x - tl.x * K, y, x + tl.x, y);
        }

        self.path_builder.close();
    }

    /// Add an arc to the path.
    pub fn arc(&mut self, params: &ArcParams) {
        let mut arc_builder = tiny_skia::PathBuilder::new();
        crate::arc::arc(&mut arc_builder, params, false);

        if let Some(path) = arc_builder.finish() {
            self.append_transformed_path(&path, self.state.transform, true, false);
        }
    }

    /// Add an arcTo segment to the path.
    pub fn arc_to(&mut self, params: &ArcToParams) {
        if !self.has_current_point {
            self.move_to(params.x1, params.y1);
            return;
        }

        let transform = self.state.transform;
        let Some(inverse) = transform.invert() else {
            log::debug!(
                target: "canvas",
                "arcTo: non-invertible transform; falling back to approximate scaling"
            );

            let (tx1, ty1) = self.transform_point(params.x1, params.y1);
            let (tx2, ty2) = self.transform_point(params.x2, params.y2);
            let t = &self.state.transform;
            let scale_x = (t.sx * t.sx + t.ky * t.ky).sqrt();
            let scale_y = (t.kx * t.kx + t.sy * t.sy).sqrt();
            let scaled_radius = params.radius * (scale_x + scale_y) / 2.0;

            crate::arc::arc_to(
                &mut self.path_builder,
                self.current_x,
                self.current_y,
                &ArcToParams {
                    x1: tx1,
                    y1: ty1,
                    x2: tx2,
                    y2: ty2,
                    radius: scaled_radius,
                },
            );
            return;
        };

        let (local_x0, local_y0) =
            Self::map_point_with_transform(&inverse, self.current_x, self.current_y);
        let mut arc_builder = tiny_skia::PathBuilder::new();
        arc_builder.move_to(local_x0, local_y0);
        crate::arc::arc_to(&mut arc_builder, local_x0, local_y0, params);

        if let Some(path) = arc_builder.finish() {
            self.append_transformed_path(&path, transform, false, true);
        }
    }

    /// Add an ellipse to the path.
    pub fn ellipse(&mut self, params: &EllipseParams) {
        // Transform center point
        let (tx, ty) = self.transform_point(params.x, params.y);
        // Scale radii
        let t = &self.state.transform;
        let scale_x = (t.sx * t.sx + t.ky * t.ky).sqrt();
        let scale_y = (t.kx * t.kx + t.sy * t.sy).sqrt();
        // Add rotation from transform to the ellipse rotation
        let transform_rotation = t.ky.atan2(t.sx);
        let total_rotation = params.rotation + transform_rotation;

        crate::arc::ellipse(
            &mut self.path_builder,
            &EllipseParams {
                x: tx,
                y: ty,
                radius_x: params.radius_x * scale_x,
                radius_y: params.radius_y * scale_y,
                rotation: total_rotation,
                start_angle: params.start_angle,
                end_angle: params.end_angle,
                anticlockwise: params.anticlockwise,
            },
            self.has_current_point,
        );
        self.has_current_point = true;
    }

    // --- Clipping ---

    /// Create a clipping region from the current path using the non-zero winding rule.
    pub fn clip(&mut self) {
        log::debug!(target: "canvas", "clip");
        self.clip_with_rule(CanvasFillRule::NonZero);
    }

    /// Create a clipping region from the current path with the specified fill rule.
    pub fn clip_with_rule(&mut self, fill_rule: CanvasFillRule) {
        log::debug!(target: "canvas", "clip_with_rule");
        let path =
            std::mem::replace(&mut self.path_builder, tiny_skia::PathBuilder::new()).finish();

        if let Some(path) = path {
            self.state.clip_path = Some(path);
            self.clip_fill_rule = fill_rule;
        }
    }

    // --- Drawing operations ---

    /// Fill the current path using the non-zero winding rule.
    pub fn fill(&mut self) {
        log::debug!(target: "canvas", "fill");
        self.fill_with_rule(CanvasFillRule::NonZero);
    }

    /// Fill the current path with the specified fill rule.
    pub fn fill_with_rule(&mut self, fill_rule: CanvasFillRule) {
        log::debug!(target: "canvas", "fill_with_rule {:?}", fill_rule);
        // Clone the path builder so we don't consume it - stroke() may follow
        let path = self.path_builder.clone().finish();

        if let Some(path) = path {
            // Create clip mask if we have a clip path
            let clip_mask = self.create_clip_mask();
            let _ = self.with_fill_paint(|ctx, paint| {
                // Path coordinates are already transformed, so use identity transform
                ctx.pixmap.fill_path(
                    &path,
                    paint,
                    fill_rule.into(),
                    Transform::identity(),
                    clip_mask.as_ref(),
                );
            });
        }
    }

    /// Stroke the current path.
    pub fn stroke(&mut self) {
        log::debug!(target: "canvas", "stroke");
        // Clone the path builder so we don't consume it - fill() may have been called or may follow
        let path = self.path_builder.clone().finish();

        if let Some(path) = path {
            // Scale line width by transform
            let t = &self.state.transform;
            let scale =
                ((t.sx * t.sx + t.ky * t.ky).sqrt() + (t.kx * t.kx + t.sy * t.sy).sqrt()) / 2.0;
            let scaled_line_width = self.state.line_width * scale;

            let stroke = tiny_skia::Stroke {
                width: scaled_line_width,
                line_cap: self.state.line_cap.into(),
                line_join: self.state.line_join.into(),
                miter_limit: self.state.miter_limit,
                dash: if self.state.line_dash.is_empty() {
                    None
                } else {
                    // Scale dash pattern too
                    let scaled_dash: Vec<f32> =
                        self.state.line_dash.iter().map(|d| d * scale).collect();
                    tiny_skia::StrokeDash::new(scaled_dash, self.state.line_dash_offset * scale)
                },
            };

            let clip_mask = self.create_clip_mask();
            let _ = self.with_stroke_paint(|ctx, paint| {
                // Path coordinates are already transformed, so use identity transform
                ctx.pixmap.stroke_path(
                    &path,
                    paint,
                    &stroke,
                    Transform::identity(),
                    clip_mask.as_ref(),
                );
            });
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
            let clip_mask = self.create_clip_mask();
            let transform = self.state.transform;
            let _ = self.with_fill_paint(|ctx, paint| {
                ctx.pixmap
                    .fill_path(p, paint, fill_rule.into(), transform, clip_mask.as_ref());
            });
        }
    }

    /// Stroke a Path2D object.
    pub fn stroke_path2d(&mut self, path: &mut Path2D) {
        if let Some(p) = path.get_path() {
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
            let transform = self.state.transform;
            let _ = self.with_stroke_paint(|ctx, paint| {
                ctx.pixmap
                    .stroke_path(p, paint, &stroke, transform, clip_mask.as_ref());
            });
        }
    }

    /// Clip to a Path2D object using the non-zero winding rule.
    pub fn clip_path2d(&mut self, path: &mut Path2D) {
        self.clip_path2d_with_rule(path, CanvasFillRule::NonZero);
    }

    /// Clip to a Path2D object with the specified fill rule.
    pub fn clip_path2d_with_rule(&mut self, path: &mut Path2D, fill_rule: CanvasFillRule) {
        if let Some(p) = path.get_path() {
            self.state.clip_path = Some(p.clone());
            self.clip_fill_rule = fill_rule;
        }
    }

    /// Fill a rectangle.
    pub fn fill_rect(&mut self, params: &RectParams) {
        log::debug!(target: "canvas", "fillRect {} {} {} {}", params.x, params.y, params.width, params.height);
        // Use path-based approach for proper transform handling
        self.begin_path();
        self.rect(params);
        self.fill();
    }

    /// Stroke a rectangle.
    pub fn stroke_rect(&mut self, params: &RectParams) {
        log::debug!(target: "canvas", "strokeRect {} {} {} {}", params.x, params.y, params.width, params.height);
        self.begin_path();
        self.rect(params);
        self.stroke();
    }

    /// Clear a rectangle (set pixels to transparent).
    pub fn clear_rect(&mut self, params: &RectParams) {
        log::debug!(target: "canvas", "clearRect {} {} {} {}", params.x, params.y, params.width, params.height);
        // Transform corners and find bounding box
        let (x0, y0) = self.transform_point(params.x, params.y);
        let (x1, y1) = self.transform_point(params.x + params.width, params.y);
        let (x2, y2) = self.transform_point(params.x + params.width, params.y + params.height);
        let (x3, y3) = self.transform_point(params.x, params.y + params.height);

        let min_x = x0.min(x1).min(x2).min(x3);
        let min_y = y0.min(y1).min(y2).min(y3);
        let max_x = x0.max(x1).max(x2).max(x3);
        let max_y = y0.max(y1).max(y2).max(y3);

        if let Some(rect) = tiny_skia::Rect::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y) {
            let paint = tiny_skia::Paint {
                blend_mode: tiny_skia::BlendMode::Clear,
                ..Default::default()
            };
            let clip_mask = self.create_clip_mask();
            self.pixmap
                .fill_rect(rect, &paint, Transform::identity(), clip_mask.as_ref());
        }
    }

    // --- Image drawing ---

    /// Internal: draw a premultiplied-alpha pixmap at (dx, dy).
    pub(crate) fn draw_image(&mut self, image: tiny_skia::PixmapRef, dx: f32, dy: f32) {
        log::debug!(target: "canvas", "drawImage {}x{} at {} {}", image.width(), image.height(), dx, dy);
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

    /// Internal: draw a premultiplied-alpha pixmap scaled.
    pub(crate) fn draw_image_scaled(
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

    /// Internal: draw a cropped region of a premultiplied-alpha pixmap.
    pub(crate) fn draw_image_cropped(
        &mut self,
        image: tiny_skia::PixmapRef,
        params: &ImageCropParams,
    ) {
        let ImageCropParams {
            sx,
            sy,
            sw,
            sh,
            dx,
            dy,
            dw,
            dh,
        } = *params;

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

    // --- Public draw image/canvas methods (backend-neutral) ---

    /// Draw image data at the specified position.
    pub fn draw_image_data(&mut self, image: &CanvasImageDataRef<'_>, dx: f32, dy: f32) {
        if let Some(pixmap) =
            tiny_skia::PixmapRef::from_bytes(image.data, image.width, image.height)
        {
            self.draw_image(pixmap, dx, dy);
        }
    }

    /// Draw image data scaled to the specified dimensions.
    pub fn draw_image_data_scaled(
        &mut self,
        image: &CanvasImageDataRef<'_>,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        if let Some(pixmap) =
            tiny_skia::PixmapRef::from_bytes(image.data, image.width, image.height)
        {
            self.draw_image_scaled(pixmap, dx, dy, dw, dh);
        }
    }

    /// Draw a cropped region of image data to a destination rectangle.
    pub fn draw_image_data_cropped(
        &mut self,
        image: &CanvasImageDataRef<'_>,
        params: &ImageCropParams,
    ) {
        if let Some(pixmap) =
            tiny_skia::PixmapRef::from_bytes(image.data, image.width, image.height)
        {
            self.draw_image_cropped(pixmap, params);
        }
    }

    /// Draw another canvas at the specified position.
    pub fn draw_canvas(&mut self, source: &Canvas2dContext, dx: f32, dy: f32) {
        self.draw_image(source.pixmap.as_ref(), dx, dy);
    }

    /// Draw another canvas scaled to the specified dimensions.
    pub fn draw_canvas_scaled(
        &mut self,
        source: &Canvas2dContext,
        dx: f32,
        dy: f32,
        dw: f32,
        dh: f32,
    ) {
        self.draw_image_scaled(source.pixmap.as_ref(), dx, dy, dw, dh);
    }

    /// Draw a cropped region of another canvas to a destination rectangle.
    pub fn draw_canvas_cropped(&mut self, source: &Canvas2dContext, params: &ImageCropParams) {
        self.draw_image_cropped(source.pixmap.as_ref(), params);
    }

    /// Create a pattern from another canvas.
    pub fn create_pattern_from_canvas(
        &self,
        source: &Canvas2dContext,
        repetition: &str,
    ) -> Canvas2dResult<Arc<CanvasPattern>> {
        let rep = repetition.parse::<Repetition>()?;
        let pattern = CanvasPattern::from_pixmap_ref(source.pixmap.as_ref(), rep)?;
        Ok(Arc::new(pattern))
    }

    // --- Transform operations ---

    /// Translate the canvas.
    pub fn translate(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "translate {} {}", x, y);
        self.state.transform = self.state.transform.pre_translate(x, y);
    }

    /// Rotate the canvas.
    pub fn rotate(&mut self, angle: f32) {
        log::debug!(target: "canvas", "rotate {}", angle);
        let cos = angle.cos();
        let sin = angle.sin();
        let rotation = Transform::from_row(cos, sin, -sin, cos, 0.0, 0.0);
        self.state.transform = self.state.transform.pre_concat(rotation);
    }

    /// Scale the canvas.
    pub fn scale(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "scale {} {}", x, y);
        self.state.transform = self.state.transform.pre_scale(x, y);
    }

    /// Apply a transform matrix.
    pub fn transform(&mut self, matrix: DOMMatrix) {
        log::debug!(target: "canvas", "transform {:?}", matrix);
        let t: Transform = matrix.into();
        self.state.transform = self.state.transform.pre_concat(t);
    }

    /// Set the transform matrix (replacing the current one).
    pub fn set_transform(&mut self, matrix: DOMMatrix) {
        log::debug!(target: "canvas", "setTransform {:?}", matrix);
        self.state.transform = matrix.into();
    }

    /// Reset the transform to identity.
    pub fn reset_transform(&mut self) {
        log::debug!(target: "canvas", "resetTransform");
        self.state.transform = Transform::identity();
    }

    /// Get the current transformation matrix.
    pub fn get_transform(&self) -> DOMMatrix {
        self.state.transform.into()
    }

    // --- Output ---

    /// Create a new ImageData with the specified dimensions.
    ///
    /// Returns a Vec<u8> filled with transparent black (all zeros).
    /// The data is in RGBA format with 4 bytes per pixel.
    pub fn create_image_data(&self, width: u32, height: u32) -> Vec<u8> {
        vec![0u8; (width * height * 4) as usize]
    }

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
            &DirtyRect {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            },
        );
    }

    /// Write a portion of image data to the canvas.
    ///
    /// The dirty rectangle specifies which portion of the source data to write.
    /// Pixels outside the canvas bounds are silently ignored.
    pub fn put_image_data_dirty(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        dx: i32,
        dy: i32,
        dirty: &DirtyRect,
    ) {
        // Clamp dirty rect to source image bounds
        let dirty_x = dirty.x.max(0).min(width as i32);
        let dirty_y = dirty.y.max(0).min(height as i32);
        let dirty_width = dirty.width.max(0).min(width as i32 - dirty_x);
        let dirty_height = dirty.height.max(0).min(height as i32 - dirty_y);

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
    ///
    /// # Arguments
    /// * `ppi` - Optional pixels per inch for PNG metadata. Defaults to 72 if not specified.
    pub fn to_png(&self, ppi: Option<f32>) -> Canvas2dResult<Vec<u8>> {
        let ppi = ppi.unwrap_or(72.0);

        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);

            // Set pixel density metadata (pixels per meter)
            let ppm = (ppi.max(0.0) / 0.0254).round() as u32;
            encoder.set_pixel_dims(Some(png::PixelDimensions {
                xppu: ppm,
                yppu: ppm,
                unit: png::Unit::Meter,
            }));

            let mut writer = encoder.write_header()?;

            // Convert from premultiplied to straight alpha for PNG
            let data = self.get_image_data(0, 0, self.width, self.height);
            writer.write_image_data(&data)?;
        }
        Ok(buf)
    }

    // --- Private helpers ---

    fn create_clip_mask(&self) -> Option<tiny_skia::Mask> {
        self.state.clip_path.as_ref().and_then(|clip_path| {
            let mut mask = tiny_skia::Mask::new(self.width, self.height)?;
            // Use identity transform because the clip path is already in pixel coordinates
            // (rect() and other path operations already transform points via transform_point())
            mask.fill_path(
                clip_path,
                self.clip_fill_rule.into(),
                true,
                Transform::identity(),
            );
            Some(mask)
        })
    }

    fn with_fill_paint<R>(
        &mut self,
        draw: impl for<'a> FnOnce(&mut Self, &tiny_skia::Paint<'a>) -> R,
    ) -> Option<R> {
        let style = self.state.fill_style.clone();
        self.with_paint_from_style(style, draw)
    }

    fn with_stroke_paint<R>(
        &mut self,
        draw: impl for<'a> FnOnce(&mut Self, &tiny_skia::Paint<'a>) -> R,
    ) -> Option<R> {
        let style = self.state.stroke_style.clone();
        self.with_paint_from_style(style, draw)
    }

    fn with_paint_from_style<R>(
        &mut self,
        style: FillStyle,
        draw: impl for<'a> FnOnce(&mut Self, &tiny_skia::Paint<'a>) -> R,
    ) -> Option<R> {
        let mut paint = tiny_skia::Paint {
            anti_alias: true,
            blend_mode: self.state.global_composite_operation,
            ..Default::default()
        };

        match style {
            FillStyle::Color(color) => {
                let mut color = color;
                // Apply global alpha
                if self.state.global_alpha < 1.0 {
                    color.set_alpha((color.alpha() * self.state.global_alpha).clamp(0.0, 1.0));
                }
                paint.set_color(color);
                Some(draw(self, &paint))
            }
            FillStyle::LinearGradient(gradient) | FillStyle::RadialGradient(gradient) => {
                let shader = self.create_gradient_shader(&gradient)?;
                paint.shader = shader;
                Some(draw(self, &paint))
            }
            FillStyle::Pattern(pattern) => {
                let canvas_width = self.pixmap.width();
                let canvas_height = self.pixmap.height();
                let (cache_width, cache_height) =
                    pattern.cache_dimensions(canvas_width, canvas_height);
                let key = PatternCacheKey {
                    pattern_id: pattern.id(),
                    repetition: pattern.repetition(),
                    canvas_width: cache_width,
                    canvas_height: cache_height,
                };
                let cached_pixmap = self.pattern_pixmap_cache.get_or_insert(key, || {
                    pattern.create_cache_pixmap(canvas_width, canvas_height)
                })?;
                let shader = pattern.create_shader_for_pixmap(
                    cached_pixmap.as_ref().as_ref(),
                    self.state.transform,
                );
                paint.shader = shader;
                Some(draw(self, &paint))
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
                let mut color: tiny_skia::Color = stop.color.into();
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
            GradientType::Radial(params) => {
                // tiny_skia's RadialGradient::new(start, end, radius, ...)
                // - start: where gradient originates (inner circle center)
                // - end: outer circle center
                // - radius: outer circle radius
                // Note: r0 (inner radius) is not directly supported by tiny_skia
                tiny_skia::RadialGradient::new(
                    tiny_skia::Point {
                        x: params.x0,
                        y: params.y0,
                    },
                    tiny_skia::Point {
                        x: params.x1,
                        y: params.y1,
                    },
                    params.r1,
                    stops,
                    tiny_skia::SpreadMode::Pad,
                    self.state.transform,
                )
            }
        }
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
    fn test_arc_to_non_invertible_transform_fallback() {
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
