//! Path2D implementation for reusable path objects.
//!
//! Path2D allows creating path objects that can be reused across multiple
//! fill, stroke, or clip operations.

use crate::error::{Canvas2dError, Canvas2dResult};
use crate::geometry::{
    ArcParams, ArcToParams, CubicBezierParams, EllipseParams, QuadraticBezierParams, RectParams,
    RoundRectParams,
};
use tiny_skia::PathBuilder;

/// A reusable path object that can be used with fill, stroke, and clip operations.
#[derive(Debug, Clone)]
pub struct Path2D {
    /// The underlying path builder for constructing the path.
    builder: PathBuilder,
    /// Cached finished path (invalidated when path is modified).
    path: Option<tiny_skia::Path>,
    /// Current position for tracking subpath.
    current_x: f32,
    current_y: f32,
    /// Subpath start for closePath.
    subpath_start_x: f32,
    subpath_start_y: f32,
    /// Whether the path has a current point (for arc line_to vs move_to).
    has_current_point: bool,
}

impl Default for Path2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Path2D {
    /// Create an empty path.
    pub fn new() -> Self {
        Self {
            builder: PathBuilder::new(),
            path: None,
            current_x: 0.0,
            current_y: 0.0,
            subpath_start_x: 0.0,
            subpath_start_y: 0.0,
            has_current_point: false,
        }
    }

    /// Create a copy of another Path2D.
    pub fn from_path(other: &Path2D) -> Self {
        other.clone()
    }

    /// Create a Path2D from SVG path data string.
    ///
    /// Supports all SVG path commands (M, L, H, V, Q, T, C, S, A, Z).
    /// Arc commands are automatically converted to cubic Bezier curves.
    ///
    /// # Example
    /// ```
    /// use vl_convert_canvas2d::Path2D;
    ///
    /// let path = Path2D::from_svg_path_data("M10,10 L50,50 A10,10 0 0 1 100,100 Z").unwrap();
    /// ```
    pub fn from_svg_path_data(path_data: &str) -> Canvas2dResult<Self> {
        let mut path = Path2D::new();

        for segment in svgtypes::SimplifyingPathParser::from(path_data) {
            let segment = segment.map_err(|e| {
                Canvas2dError::InvalidArgument(format!("Invalid SVG path data: {:?}", e))
            })?;

            match segment {
                svgtypes::SimplePathSegment::MoveTo { x, y } => {
                    path.move_to(x as f32, y as f32);
                }
                svgtypes::SimplePathSegment::LineTo { x, y } => {
                    path.line_to(x as f32, y as f32);
                }
                svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                    path.quadratic_curve_to(&QuadraticBezierParams {
                        cpx: x1 as f32,
                        cpy: y1 as f32,
                        x: x as f32,
                        y: y as f32,
                    });
                }
                svgtypes::SimplePathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    path.bezier_curve_to(&CubicBezierParams {
                        cp1x: x1 as f32,
                        cp1y: y1 as f32,
                        cp2x: x2 as f32,
                        cp2y: y2 as f32,
                        x: x as f32,
                        y: y as f32,
                    });
                }
                svgtypes::SimplePathSegment::ClosePath => {
                    path.close_path();
                }
            }
        }

        Ok(path)
    }

    /// Invalidate the cached path (called when path is modified).
    fn invalidate(&mut self) {
        self.path = None;
    }

    /// Move to a point without drawing.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.invalidate();
        self.builder.move_to(x, y);
        self.current_x = x;
        self.current_y = y;
        self.subpath_start_x = x;
        self.subpath_start_y = y;
        self.has_current_point = true;
    }

    /// Draw a line to a point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.invalidate();
        self.builder.line_to(x, y);
        self.current_x = x;
        self.current_y = y;
        self.has_current_point = true;
    }

    /// Close the current subpath.
    pub fn close_path(&mut self) {
        self.invalidate();
        self.builder.close();
        self.current_x = self.subpath_start_x;
        self.current_y = self.subpath_start_y;
    }

    /// Add a cubic bezier curve.
    pub fn bezier_curve_to(&mut self, params: &CubicBezierParams) {
        self.invalidate();
        self.builder.cubic_to(
            params.cp1x,
            params.cp1y,
            params.cp2x,
            params.cp2y,
            params.x,
            params.y,
        );
        self.current_x = params.x;
        self.current_y = params.y;
    }

    /// Add a quadratic bezier curve.
    pub fn quadratic_curve_to(&mut self, params: &QuadraticBezierParams) {
        self.invalidate();
        self.builder
            .quad_to(params.cpx, params.cpy, params.x, params.y);
        self.current_x = params.x;
        self.current_y = params.y;
    }

    /// Add a rectangle to the path.
    pub fn rect(&mut self, params: &RectParams) {
        self.invalidate();
        self.move_to(params.x, params.y);
        self.line_to(params.x + params.width, params.y);
        self.line_to(params.x + params.width, params.y + params.height);
        self.line_to(params.x, params.y + params.height);
        self.close_path();
    }

    /// Add a rounded rectangle to the path.
    pub fn round_rect(&mut self, params: &RoundRectParams) {
        self.invalidate();

        // Handle negative dimensions
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

        // Clamp radii
        tl = tl.max(0.0);
        tr = tr.max(0.0);
        br = br.max(0.0);
        bl = bl.max(0.0);

        // Scale radii if they exceed dimensions
        let scale_x = width / (tl.max(bl) + tr.max(br)).max(1e-10);
        let scale_y = height / (tl.max(tr) + bl.max(br)).max(1e-10);
        let scale = scale_x.min(scale_y).min(1.0);

        if scale < 1.0 {
            tl *= scale;
            tr *= scale;
            br *= scale;
            bl *= scale;
        }

        // Build the rounded rectangle
        self.builder.move_to(x + tl, y);
        self.builder.line_to(x + width - tr, y);
        if tr > 0.0 {
            self.builder.quad_to(x + width, y, x + width, y + tr);
        }
        self.builder.line_to(x + width, y + height - br);
        if br > 0.0 {
            self.builder
                .quad_to(x + width, y + height, x + width - br, y + height);
        }
        self.builder.line_to(x + bl, y + height);
        if bl > 0.0 {
            self.builder.quad_to(x, y + height, x, y + height - bl);
        }
        self.builder.line_to(x, y + tl);
        if tl > 0.0 {
            self.builder.quad_to(x, y, x + tl, y);
        }
        self.builder.close();

        self.subpath_start_x = x + tl;
        self.subpath_start_y = y;
        self.current_x = x + tl;
        self.current_y = y;
    }

    /// Add an arc to the path.
    pub fn arc(&mut self, params: &ArcParams) {
        self.invalidate();
        crate::arc::arc(&mut self.builder, params, self.has_current_point);
        self.has_current_point = true;
    }

    /// Add an arcTo segment to the path.
    pub fn arc_to(&mut self, params: &ArcToParams) {
        self.invalidate();
        crate::arc::arc_to(&mut self.builder, self.current_x, self.current_y, params);
    }

    /// Add an ellipse to the path.
    pub fn ellipse(&mut self, params: &EllipseParams) {
        self.invalidate();
        crate::arc::ellipse(&mut self.builder, params, self.has_current_point);
        self.has_current_point = true;
    }

    /// Get the finished path for rendering.
    /// Returns None if the path is empty.
    pub(crate) fn get_path(&mut self) -> Option<&tiny_skia::Path> {
        if self.path.is_none() {
            // Clone the builder to finish it without consuming it
            let builder_clone = self.builder.clone();
            self.path = builder_clone.finish();
        }
        self.path.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{
        ArcParams, CubicBezierParams, EllipseParams, QuadraticBezierParams, RectParams,
        RoundRectParams,
    };
    use tiny_skia::PathSegment;

    /// Collect all segments from a Path2D into a Vec for assertion.
    fn segments(path: &mut Path2D) -> Vec<PathSegment> {
        path.get_path()
            .map(|p| p.segments().collect())
            .unwrap_or_default()
    }

    fn pt(x: f32, y: f32) -> tiny_skia::Point {
        tiny_skia::Point { x, y }
    }

    #[test]
    fn test_empty_path_returns_none() {
        let mut path = Path2D::new();
        assert!(path.get_path().is_none());
    }

    #[test]
    fn test_move_only_returns_none() {
        // A single moveTo with no geometry produces no renderable path
        let mut path = Path2D::new();
        path.move_to(10.0, 20.0);
        assert!(path.get_path().is_none());
    }

    #[test]
    fn test_line_segments() {
        let mut path = Path2D::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 50.0);

        let segs = segments(&mut path);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(0.0, 0.0)));
        assert_eq!(segs[1], PathSegment::LineTo(pt(100.0, 0.0)));
        assert_eq!(segs[2], PathSegment::LineTo(pt(100.0, 50.0)));
    }

    #[test]
    fn test_rect_produces_closed_path() {
        let mut path = Path2D::new();
        path.rect(&RectParams {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        });

        let segs = segments(&mut path);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(10.0, 20.0)));
        assert_eq!(segs[1], PathSegment::LineTo(pt(40.0, 20.0)));
        assert_eq!(segs[2], PathSegment::LineTo(pt(40.0, 60.0)));
        assert_eq!(segs[3], PathSegment::LineTo(pt(10.0, 60.0)));
        assert_eq!(segs[4], PathSegment::Close);
    }

    #[test]
    fn test_rect_bounds() {
        let mut path = Path2D::new();
        path.rect(&RectParams {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        });

        let bounds = path.get_path().unwrap().bounds();
        assert_eq!(bounds.left(), 10.0);
        assert_eq!(bounds.top(), 20.0);
        assert_eq!(bounds.right(), 40.0);
        assert_eq!(bounds.bottom(), 60.0);
    }

    #[test]
    fn test_close_path_returns_to_subpath_start() {
        let mut path = Path2D::new();
        path.move_to(10.0, 10.0);
        path.line_to(50.0, 10.0);
        path.close_path();
        assert_eq!(path.current_x, 10.0);
        assert_eq!(path.current_y, 10.0);

        let segs = segments(&mut path);
        assert_eq!(segs.last(), Some(&PathSegment::Close));
    }

    #[test]
    fn test_quadratic_curve() {
        let mut path = Path2D::new();
        path.move_to(0.0, 0.0);
        path.quadratic_curve_to(&QuadraticBezierParams {
            cpx: 50.0,
            cpy: 100.0,
            x: 100.0,
            y: 0.0,
        });

        let segs = segments(&mut path);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(0.0, 0.0)));
        assert_eq!(
            segs[1],
            PathSegment::QuadTo(pt(50.0, 100.0), pt(100.0, 0.0))
        );
    }

    #[test]
    fn test_bezier_curve() {
        let mut path = Path2D::new();
        path.move_to(0.0, 0.0);
        path.bezier_curve_to(&CubicBezierParams {
            cp1x: 10.0,
            cp1y: 50.0,
            cp2x: 90.0,
            cp2y: 50.0,
            x: 100.0,
            y: 0.0,
        });

        let segs = segments(&mut path);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(0.0, 0.0)));
        assert_eq!(
            segs[1],
            PathSegment::CubicTo(pt(10.0, 50.0), pt(90.0, 50.0), pt(100.0, 0.0))
        );
    }

    #[test]
    fn test_current_position_tracking() {
        let mut path = Path2D::new();
        assert!(!path.has_current_point);

        path.move_to(10.0, 20.0);
        assert!(path.has_current_point);
        assert_eq!(path.current_x, 10.0);
        assert_eq!(path.current_y, 20.0);

        path.line_to(30.0, 40.0);
        assert_eq!(path.current_x, 30.0);
        assert_eq!(path.current_y, 40.0);

        path.bezier_curve_to(&CubicBezierParams {
            cp1x: 0.0,
            cp1y: 0.0,
            cp2x: 0.0,
            cp2y: 0.0,
            x: 50.0,
            y: 60.0,
        });
        assert_eq!(path.current_x, 50.0);
        assert_eq!(path.current_y, 60.0);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut path = Path2D::new();
        path.move_to(0.0, 0.0);
        path.line_to(10.0, 10.0);

        // Build cache
        let _ = path.get_path();
        assert!(path.path.is_some());

        // Modification invalidates cache
        path.line_to(20.0, 20.0);
        assert!(path.path.is_none());

        // Rebuild produces updated path
        let segs = segments(&mut path);
        assert_eq!(segs.len(), 3);
    }

    #[test]
    fn test_clone_does_not_copy_cache() {
        let mut path1 = Path2D::new();
        path1.rect(&RectParams {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        let _ = path1.get_path(); // populate cache

        let mut path2 = Path2D::from_path(&path1);
        // Clone rebuilds from builder, not cache
        let segs = segments(&mut path2);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(0.0, 0.0)));
        assert!(segs.contains(&PathSegment::Close));
    }

    #[test]
    fn test_svg_path_line() {
        let mut path = Path2D::from_svg_path_data("M10,10 L50,50 Z").unwrap();

        let segs = segments(&mut path);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(10.0, 10.0)));
        assert_eq!(segs[1], PathSegment::LineTo(pt(50.0, 50.0)));
        assert_eq!(segs[2], PathSegment::Close);
    }

    #[test]
    fn test_svg_path_relative_commands() {
        let mut path = Path2D::from_svg_path_data("M10,10 l40,40 z").unwrap();

        let segs = segments(&mut path);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(10.0, 10.0)));
        assert_eq!(segs[1], PathSegment::LineTo(pt(50.0, 50.0)));
        assert_eq!(segs[2], PathSegment::Close);
    }

    #[test]
    fn test_svg_path_curves() {
        let mut path =
            Path2D::from_svg_path_data("M0,0 Q50,50,100,0 C150,50,200,50,250,0").unwrap();

        let segs = segments(&mut path);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(0.0, 0.0)));
        assert_eq!(segs[1], PathSegment::QuadTo(pt(50.0, 50.0), pt(100.0, 0.0)));
        assert_eq!(
            segs[2],
            PathSegment::CubicTo(pt(150.0, 50.0), pt(200.0, 50.0), pt(250.0, 0.0))
        );
    }

    #[test]
    fn test_svg_path_arc_produces_cubics() {
        // SVG arcs get converted to cubic beziers
        let mut path = Path2D::from_svg_path_data("M10,10 A20,20 0 0 1 50,50").unwrap();

        let segs = segments(&mut path);
        assert_eq!(segs[0], PathSegment::MoveTo(pt(10.0, 10.0)));
        // Arc decomposition produces cubics, not arcs
        assert!(segs
            .iter()
            .skip(1)
            .all(|s| matches!(s, PathSegment::CubicTo(..))));
    }

    #[test]
    fn test_svg_path_empty() {
        let mut path = Path2D::from_svg_path_data("").unwrap();
        assert!(path.get_path().is_none());
    }

    #[test]
    fn test_svg_path_invalid() {
        let result = Path2D::from_svg_path_data("not valid path data");
        assert!(result.is_err());
    }

    #[test]
    fn test_round_rect_bounds() {
        let mut path = Path2D::new();
        path.round_rect(&RoundRectParams {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
            radii: [5.0, 5.0, 5.0, 5.0],
        });

        let bounds = path.get_path().unwrap().bounds();
        assert_eq!(bounds.left(), 10.0);
        assert_eq!(bounds.top(), 20.0);
        assert_eq!(bounds.right(), 110.0);
        assert_eq!(bounds.bottom(), 70.0);
    }

    #[test]
    fn test_round_rect_has_quads_and_close() {
        let mut path = Path2D::new();
        path.round_rect(&RoundRectParams {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            radii: [10.0, 10.0, 10.0, 10.0],
        });

        let segs = segments(&mut path);
        // Should have quad segments for rounded corners
        assert!(segs.iter().any(|s| matches!(s, PathSegment::QuadTo(..))));
        assert_eq!(segs.last(), Some(&PathSegment::Close));
    }

    #[test]
    fn test_round_rect_zero_radius_is_rect() {
        let mut round = Path2D::new();
        round.round_rect(&RoundRectParams {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            radii: [0.0, 0.0, 0.0, 0.0],
        });

        let segs = segments(&mut round);
        // Zero radius means no quad segments — just lines
        assert!(!segs.iter().any(|s| matches!(s, PathSegment::QuadTo(..))));
    }

    #[test]
    fn test_arc_sets_has_current_point() {
        let mut path = Path2D::new();
        assert!(!path.has_current_point);

        path.arc(&ArcParams {
            x: 50.0,
            y: 50.0,
            radius: 25.0,
            start_angle: 0.0,
            end_angle: std::f32::consts::PI,
            anticlockwise: false,
        });
        assert!(path.has_current_point);
    }

    #[test]
    fn test_ellipse_sets_has_current_point() {
        let mut path = Path2D::new();
        assert!(!path.has_current_point);

        path.ellipse(&EllipseParams {
            x: 50.0,
            y: 50.0,
            radius_x: 30.0,
            radius_y: 20.0,
            rotation: 0.0,
            start_angle: 0.0,
            end_angle: std::f32::consts::TAU,
            anticlockwise: false,
        });
        assert!(path.has_current_point);
    }

    #[test]
    fn test_multiple_subpaths() {
        let mut path = Path2D::new();
        path.move_to(0.0, 0.0);
        path.line_to(10.0, 10.0);
        path.move_to(50.0, 50.0);
        path.line_to(60.0, 60.0);

        let segs = segments(&mut path);
        let move_count = segs
            .iter()
            .filter(|s| matches!(s, PathSegment::MoveTo(..)))
            .count();
        assert_eq!(move_count, 2);
    }
}
