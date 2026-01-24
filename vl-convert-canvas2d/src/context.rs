//! Canvas 2D rendering context implementation.

use crate::error::{Canvas2dError, Canvas2dResult};
use crate::font_parser::{parse_font, ParsedFont};
use crate::gradient::{CanvasGradient, GradientType};
use crate::style::{FillStyle, LineCap, LineJoin, TextAlign, TextBaseline};
use crate::text::TextMetrics;
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use tiny_skia::{Pixmap, Transform};

/// Maximum canvas dimension (same as Chrome).
const MAX_DIMENSION: u32 = 32767;

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

        Ok(Self {
            width,
            height,
            pixmap,
            font_system,
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

    /// Measure text and return metrics.
    pub fn measure_text(&mut self, text: &str) -> Canvas2dResult<TextMetrics> {
        crate::text::measure_text(&mut self.font_system, text, &self.state.font)
    }

    /// Fill text at the specified position.
    pub fn fill_text(&mut self, text: &str, x: f32, y: f32) {
        self.render_text(text, x, y, true);
    }

    /// Stroke text at the specified position.
    pub fn stroke_text(&mut self, text: &str, x: f32, y: f32) {
        self.render_text(text, x, y, false);
    }

    /// Internal text rendering (used by fillText and strokeText).
    fn render_text(&mut self, text: &str, x: f32, y: f32, fill: bool) {
        let font = &self.state.font;
        let metrics = Metrics::new(font.size_px, font.size_px * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        // Build attributes from parsed font
        let family = font
            .families
            .first()
            .map(|f| Family::Name(f))
            .unwrap_or(Family::SansSerif);

        let attrs = Attrs::new()
            .family(family)
            .weight(font.weight)
            .style(font.style);

        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Get text width for alignment
        let mut text_width: f32 = 0.0;
        for run in buffer.layout_runs() {
            text_width = text_width.max(run.line_w);
        }

        // Calculate alignment offset
        let x_offset = crate::text::calculate_text_x_offset(text_width, self.state.text_align);

        // Calculate baseline offset
        let y_offset = crate::text::calculate_text_y_offset(font.size_px, self.state.text_baseline);

        // Get the paint
        let paint = if fill {
            self.create_fill_paint()
        } else {
            self.create_stroke_paint()
        };

        let Some(paint) = paint else {
            return;
        };

        // Render each glyph
        // Note: This is a simplified implementation that renders text as rectangles
        // for each glyph position. Full text rendering would require integrating
        // with swash for rasterization or using a different approach.
        // For now, we create a path for each glyph's bounding box.
        let final_x = x + x_offset;
        let final_y = y + y_offset;

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                // Create a small filled rectangle at each glyph position
                // This is a placeholder - proper text rendering would rasterize the glyphs
                let glyph_x = final_x + glyph.x;
                let glyph_y = final_y - font.size_px * 0.8 + run.line_y;
                let glyph_w = glyph.w;
                let glyph_h = font.size_px;

                if fill {
                    if let Some(rect) =
                        tiny_skia::Rect::from_xywh(glyph_x, glyph_y, glyph_w, glyph_h)
                    {
                        self.pixmap
                            .fill_rect(rect, &paint, self.state.transform, None);
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

    /// Create a clipping region from the current path.
    pub fn clip(&mut self) {
        let path =
            std::mem::replace(&mut self.path_builder, tiny_skia::PathBuilder::new()).finish();

        if let Some(path) = path {
            self.state.clip_path = Some(path);
        }
    }

    /// Check if a point is inside the current path.
    /// Note: This is a simplified implementation - full implementation would need
    /// proper point-in-polygon testing.
    pub fn is_point_in_path(&self, _x: f32, _y: f32) -> bool {
        // tiny_skia doesn't have built-in hit testing, so we return false as a placeholder
        // Full implementation would require point-in-polygon algorithm
        false
    }

    // --- Drawing operations ---

    /// Fill the current path.
    pub fn fill(&mut self) {
        let path =
            std::mem::replace(&mut self.path_builder, tiny_skia::PathBuilder::new()).finish();

        if let Some(path) = path {
            if let Some(paint) = self.create_fill_paint() {
                // Create clip mask if we have a clip path
                let clip_mask = self.create_clip_mask();
                self.pixmap.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
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

    /// Fill a rectangle.
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
            if let Some(paint) = self.create_fill_paint() {
                self.pixmap
                    .fill_rect(rect, &paint, self.state.transform, None);
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
            self.pixmap
                .fill_rect(rect, &paint, self.state.transform, None);
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
            FillStyle::Pattern => {
                // Pattern not implemented yet
                None
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
                // Note: tiny_skia's RadialGradient is center-based, not two-point
                // We use the outer circle (x1, y1, r1) as the gradient basis
                // r0 (inner radius) is not directly supported by tiny_skia
                tiny_skia::RadialGradient::new(
                    tiny_skia::Point { x: *x1, y: *y1 },
                    tiny_skia::Point {
                        x: *x0 - *x1,
                        y: *y0 - *y1,
                    },
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
