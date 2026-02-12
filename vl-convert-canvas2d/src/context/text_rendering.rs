//! Text rendering operations for Canvas2dContext.

use super::Canvas2dContext;
use crate::error::Canvas2dResult;
use crate::font_parser::parse_font;
use crate::style::{FontStretch, TextAlign, TextBaseline};
use crate::text::TextMetrics;
use cosmic_text::{Attrs, Buffer, CacheKeyFlags, Command, Family, Metrics, Shaping};
use tiny_skia::Transform;

impl Canvas2dContext {
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
}
