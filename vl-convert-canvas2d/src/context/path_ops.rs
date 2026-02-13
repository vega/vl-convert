//! Path building operations for Canvas2dContext.

use super::Canvas2dContext;
use crate::geometry::{
    ArcParams, ArcToParams, CubicBezierParams, EllipseParams, QuadraticBezierParams, RectParams,
    RoundRectParams,
};
use tiny_skia::{PathSegment, Transform};

impl Canvas2dContext {
    /// Begin a new path.
    pub fn begin_path(&mut self) {
        log::debug!(target: "canvas", "beginPath");
        self.path_builder = tiny_skia::PathBuilder::new();
        self.has_current_point = false;
    }

    /// Transform a point by the current transformation matrix.
    /// Canvas 2D spec requires path coordinates to be transformed when added to the path.
    pub(crate) fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        Self::map_point_with_transform(&self.state.transform, x, y)
    }

    pub(crate) fn map_point_with_transform(transform: &Transform, x: f32, y: f32) -> (f32, f32) {
        (
            transform.sx * x + transform.kx * y + transform.tx,
            transform.ky * x + transform.sy * y + transform.ty,
        )
    }

    /// Append all segments from a finished path to the current path builder,
    /// transforming each point by the given transform.
    ///
    /// Used by arc/arc_to/ellipse/round_rect to merge temp-built paths into
    /// the main path with pre-transformation applied.
    pub(crate) fn append_transformed_path(
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

        // Build the round rect in user space, then transform all points to device space.
        // This correctly handles all transforms including rotation and non-uniform scaling.
        let mut temp = tiny_skia::PathBuilder::new();

        temp.move_to(x + tl.x, y);

        // Top edge
        temp.line_to(x + width - tr.x, y);

        // Top-right corner
        if tr.x > 0.0 || tr.y > 0.0 {
            temp.cubic_to(
                x + width - tr.x + tr.x * K,
                y,
                x + width,
                y + tr.y - tr.y * K,
                x + width,
                y + tr.y,
            );
        }

        // Right edge
        temp.line_to(x + width, y + height - br.y);

        // Bottom-right corner
        if br.x > 0.0 || br.y > 0.0 {
            temp.cubic_to(
                x + width,
                y + height - br.y + br.y * K,
                x + width - br.x + br.x * K,
                y + height,
                x + width - br.x,
                y + height,
            );
        }

        // Bottom edge
        temp.line_to(x + bl.x, y + height);

        // Bottom-left corner
        if bl.x > 0.0 || bl.y > 0.0 {
            temp.cubic_to(
                x + bl.x - bl.x * K,
                y + height,
                x,
                y + height - bl.y + bl.y * K,
                x,
                y + height - bl.y,
            );
        }

        // Left edge
        temp.line_to(x, y + tl.y);

        // Top-left corner
        if tl.x > 0.0 || tl.y > 0.0 {
            temp.cubic_to(x, y + tl.y - tl.y * K, x + tl.x - tl.x * K, y, x + tl.x, y);
        }

        temp.close();

        if let Some(path) = temp.finish() {
            self.append_transformed_path(&path, self.state.transform, false, false);
        }
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
            // Non-invertible transform: transform control points and approximate radius
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

        // current_x/current_y are in device space; map back to user space for arc_to
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
        let mut ellipse_builder = tiny_skia::PathBuilder::new();
        crate::arc::ellipse(&mut ellipse_builder, params, false);

        if let Some(path) = ellipse_builder.finish() {
            self.append_transformed_path(
                &path,
                self.state.transform,
                self.has_current_point,
                false,
            );
        }
    }
}
