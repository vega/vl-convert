//! Path building operations for Canvas2dContext.

use super::Canvas2dContext;
use crate::geometry::{
    ArcParams, ArcToParams, CubicBezierParams, EllipseParams, QuadraticBezierParams, RectParams,
    RoundRectParams,
};
use tiny_skia::PathSegment;

impl Canvas2dContext {
    /// Begin a new path.
    pub fn begin_path(&mut self) {
        log::debug!(target: "canvas", "beginPath");
        self.path_builder = tiny_skia::PathBuilder::new();
        self.has_current_point = false;
    }

    /// Append all segments from a finished path to the current path builder.
    ///
    /// Used by arc/arc_to/ellipse to merge temp-built paths into the main path.
    /// Updates `current_x`/`current_y`/`has_current_point` from the appended segments.
    fn append_path_segments(&mut self, path: &tiny_skia::Path, connect_first: bool) {
        let mut first_move = true;
        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    if first_move && connect_first && self.has_current_point {
                        // Per Canvas 2D spec: arc/ellipse connects to current point via line
                        self.path_builder.line_to(p.x, p.y);
                    } else {
                        self.path_builder.move_to(p.x, p.y);
                        self.subpath_start_x = p.x;
                        self.subpath_start_y = p.y;
                    }
                    first_move = false;
                    self.current_x = p.x;
                    self.current_y = p.y;
                    self.has_current_point = true;
                }
                PathSegment::LineTo(p) => {
                    self.path_builder.line_to(p.x, p.y);
                    self.current_x = p.x;
                    self.current_y = p.y;
                    self.has_current_point = true;
                }
                PathSegment::QuadTo(ctrl, p) => {
                    self.path_builder.quad_to(ctrl.x, ctrl.y, p.x, p.y);
                    self.current_x = p.x;
                    self.current_y = p.y;
                    self.has_current_point = true;
                }
                PathSegment::CubicTo(ctrl1, ctrl2, p) => {
                    self.path_builder
                        .cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, p.x, p.y);
                    self.current_x = p.x;
                    self.current_y = p.y;
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
        self.path_builder.move_to(x, y);
        self.current_x = x;
        self.current_y = y;
        self.subpath_start_x = x;
        self.subpath_start_y = y;
        self.has_current_point = true;
    }

    /// Draw a line to a point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "lineTo {} {}", x, y);
        self.path_builder.line_to(x, y);
        self.current_x = x;
        self.current_y = y;
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
        self.path_builder.cubic_to(
            params.cp1x, params.cp1y, params.cp2x, params.cp2y, params.x, params.y,
        );
        self.current_x = params.x;
        self.current_y = params.y;
        self.has_current_point = true;
    }

    /// Add a quadratic bezier curve.
    pub fn quadratic_curve_to(&mut self, params: &QuadraticBezierParams) {
        self.path_builder
            .quad_to(params.cpx, params.cpy, params.x, params.y);
        self.current_x = params.x;
        self.current_y = params.y;
        self.has_current_point = true;
    }

    /// Add a rectangle to the path.
    pub fn rect(&mut self, params: &RectParams) {
        log::debug!(target: "canvas", "rect {} {} {} {}", params.x, params.y, params.width, params.height);
        let x = params.x;
        let y = params.y;
        let w = params.width;
        let h = params.height;

        self.path_builder.move_to(x, y);
        self.path_builder.line_to(x + w, y);
        self.path_builder.line_to(x + w, y + h);
        self.path_builder.line_to(x, y + h);
        self.path_builder.close();

        self.current_x = x;
        self.current_y = y;
        self.subpath_start_x = x;
        self.subpath_start_y = y;
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
            self.append_path_segments(&path, true);
        }
    }

    /// Add an arcTo segment to the path.
    pub fn arc_to(&mut self, params: &ArcToParams) {
        if !self.has_current_point {
            self.move_to(params.x1, params.y1);
            return;
        }

        let mut arc_builder = tiny_skia::PathBuilder::new();
        arc_builder.move_to(self.current_x, self.current_y);
        crate::arc::arc_to(
            &mut arc_builder,
            self.current_x,
            self.current_y,
            params,
        );

        if let Some(path) = arc_builder.finish() {
            // Skip the initial MoveTo (which is just the current point) and append the rest
            let mut first_move = true;
            for segment in path.segments() {
                match segment {
                    PathSegment::MoveTo(_) if first_move => {
                        first_move = false;
                        // Skip — this is just the current point we already have
                    }
                    PathSegment::MoveTo(p) => {
                        self.path_builder.move_to(p.x, p.y);
                        self.subpath_start_x = p.x;
                        self.subpath_start_y = p.y;
                        self.current_x = p.x;
                        self.current_y = p.y;
                    }
                    PathSegment::LineTo(p) => {
                        self.path_builder.line_to(p.x, p.y);
                        self.current_x = p.x;
                        self.current_y = p.y;
                    }
                    PathSegment::QuadTo(ctrl, p) => {
                        self.path_builder.quad_to(ctrl.x, ctrl.y, p.x, p.y);
                        self.current_x = p.x;
                        self.current_y = p.y;
                    }
                    PathSegment::CubicTo(ctrl1, ctrl2, p) => {
                        self.path_builder
                            .cubic_to(ctrl1.x, ctrl1.y, ctrl2.x, ctrl2.y, p.x, p.y);
                        self.current_x = p.x;
                        self.current_y = p.y;
                    }
                    PathSegment::Close => {
                        self.path_builder.close();
                        self.current_x = self.subpath_start_x;
                        self.current_y = self.subpath_start_y;
                    }
                }
                self.has_current_point = true;
            }
        }
    }

    /// Add an ellipse to the path.
    pub fn ellipse(&mut self, params: &EllipseParams) {
        let mut ellipse_builder = tiny_skia::PathBuilder::new();
        crate::arc::ellipse(&mut ellipse_builder, params, false);

        if let Some(path) = ellipse_builder.finish() {
            self.append_path_segments(&path, self.has_current_point);
        }
    }
}
