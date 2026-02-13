//! Fill, stroke, clip, and paint helper operations for Canvas2dContext.

use super::Canvas2dContext;
use crate::geometry::RectParams;
use crate::gradient::{CanvasGradient, GradientType};
use crate::path2d::Path2D;
use crate::pattern_cache::PatternCacheKey;
use crate::style::{CanvasFillRule, FillStyle};
use tiny_skia::Transform;

impl Canvas2dContext {
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
            // Inline path coordinates are pre-transformed to device space
            self.state.clip_transform = Transform::identity();
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
            let clip_mask = self.create_clip_mask();
            // Path coordinates are already in device space (pre-transformed)
            let _ = self.with_fill_paint(|ctx, paint| {
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
            // Scale line width and dash pattern by the average axis scale of the CTM,
            // since path coordinates are pre-transformed but stroke width is in user space
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
                    let scaled_dash: Vec<f32> =
                        self.state.line_dash.iter().map(|d| d * scale).collect();
                    tiny_skia::StrokeDash::new(scaled_dash, self.state.line_dash_offset * scale)
                },
            };

            let clip_mask = self.create_clip_mask();
            // Path coordinates are already in device space (pre-transformed)
            let _ = self.with_stroke_paint(|ctx, paint| {
                ctx.pixmap
                    .stroke_path(&path, paint, &stroke, Transform::identity(), clip_mask.as_ref());
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
            self.state.clip_transform = self.state.transform;
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
        // Transform corners to device space
        let (x0, y0) = self.transform_point(params.x, params.y);
        let (x1, y1) = self.transform_point(params.x + params.width, params.y);
        let (x2, y2) = self.transform_point(params.x + params.width, params.y + params.height);
        let (x3, y3) = self.transform_point(params.x, params.y + params.height);

        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        pb.line_to(x2, y2);
        pb.line_to(x3, y3);
        pb.close();

        if let Some(path) = pb.finish() {
            let paint = tiny_skia::Paint {
                blend_mode: tiny_skia::BlendMode::Clear,
                ..Default::default()
            };
            let clip_mask = self.create_clip_mask();
            self.pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                clip_mask.as_ref(),
            );
        }
    }

    // --- Private paint helpers ---

    pub(crate) fn create_clip_mask(&self) -> Option<tiny_skia::Mask> {
        self.state.clip_path.as_ref().and_then(|clip_path| {
            let mut mask = tiny_skia::Mask::new(self.width, self.height)?;
            mask.fill_path(
                clip_path,
                self.clip_fill_rule.into(),
                true,
                self.state.clip_transform,
            );
            Some(mask)
        })
    }

    pub(crate) fn with_fill_paint<R>(
        &mut self,
        draw: impl for<'a> FnOnce(&mut Self, &tiny_skia::Paint<'a>) -> R,
    ) -> Option<R> {
        let style = self.state.fill_style.clone();
        self.with_paint_from_style(style, draw)
    }

    pub(crate) fn with_stroke_paint<R>(
        &mut self,
        draw: impl for<'a> FnOnce(&mut Self, &tiny_skia::Paint<'a>) -> R,
    ) -> Option<R> {
        let style = self.state.stroke_style.clone();
        self.with_paint_from_style(style, draw)
    }

    pub(crate) fn with_paint_from_style<R>(
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

    pub(crate) fn create_gradient_shader(
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
}
