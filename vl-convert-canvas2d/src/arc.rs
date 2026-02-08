//! Arc operations using bezier curve approximation.
//!
//! tiny-skia does not support arc primitives directly, so we approximate
//! arcs using cubic bezier curves.

use std::f32::consts::PI;
use tiny_skia::PathBuilder;

use crate::geometry::{ArcParams, ArcToParams, EllipseParams};

/// Add an arc to the path using bezier curve approximation.
///
/// Delegates to [`ellipse`] with equal radii and no rotation.
pub fn arc(path: &mut PathBuilder, params: &ArcParams, has_current_point: bool) {
    ellipse(path, &EllipseParams::from(params), has_current_point);
}

/// Add an elliptical arc to the path using bezier curve approximation.
pub fn ellipse(path: &mut PathBuilder, params: &EllipseParams, has_current_point: bool) {
    let EllipseParams {
        x,
        y,
        radius_x,
        radius_y,
        rotation,
        start_angle,
        end_angle,
        anticlockwise,
    } = *params;

    if radius_x <= 0.0 || radius_y <= 0.0 {
        return;
    }

    // Normalize angles
    let (mut start, mut end) = (start_angle, end_angle);

    if anticlockwise {
        std::mem::swap(&mut start, &mut end);
    }

    // Ensure end > start
    while end < start {
        end += 2.0 * PI;
    }

    // Calculate number of segments (use more segments for larger arcs)
    let angle_span = end - start;
    let num_segments = ((angle_span / (PI / 2.0)).ceil() as usize).max(1);
    let segment_angle = angle_span / num_segments as f32;

    // Precompute rotation matrix
    let cos_rot = rotation.cos();
    let sin_rot = rotation.sin();

    // Start point
    let start_x = x + radius_x * start.cos() * cos_rot - radius_y * start.sin() * sin_rot;
    let start_y = y + radius_x * start.cos() * sin_rot + radius_y * start.sin() * cos_rot;

    // Per Canvas 2D spec: if there's a current point, line to the arc start;
    // otherwise move to the arc start.
    if has_current_point {
        path.line_to(start_x, start_y);
    } else {
        path.move_to(start_x, start_y);
    }

    // Draw arc segments
    for i in 0..num_segments {
        let angle1 = start + i as f32 * segment_angle;
        let angle2 = start + (i + 1) as f32 * segment_angle;

        arc_segment(
            path, x, y, radius_x, radius_y, cos_rot, sin_rot, angle1, angle2,
        );
    }
}

/// Add a single arc segment as a cubic bezier curve.
#[allow(clippy::too_many_arguments)]
fn arc_segment(
    path: &mut PathBuilder,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    cos_rot: f32,
    sin_rot: f32,
    angle1: f32,
    angle2: f32,
) {
    // Calculate bezier control point factor
    let angle_diff = angle2 - angle1;
    let k = 4.0 / 3.0 * (angle_diff / 4.0).tan();

    // Points on the unit circle
    let x1 = angle1.cos();
    let y1 = angle1.sin();
    let x2 = angle2.cos();
    let y2 = angle2.sin();

    // Control points on the unit circle
    let cp1x = x1 - k * y1;
    let cp1y = y1 + k * x1;
    let cp2x = x2 + k * y2;
    let cp2y = y2 - k * x2;

    // Transform points
    let transform_point = |px: f32, py: f32| -> (f32, f32) {
        let tx = rx * px;
        let ty = ry * py;
        (
            cx + tx * cos_rot - ty * sin_rot,
            cy + tx * sin_rot + ty * cos_rot,
        )
    };

    let (ctrl1_x, ctrl1_y) = transform_point(cp1x, cp1y);
    let (ctrl2_x, ctrl2_y) = transform_point(cp2x, cp2y);
    let (end_x, end_y) = transform_point(x2, y2);

    path.cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, end_x, end_y);
}

/// Add an arc connecting two points with a given radius (arcTo operation).
///
/// `x0, y0` is the current point (path state, not geometry).
pub fn arc_to(path: &mut PathBuilder, x0: f32, y0: f32, params: &ArcToParams) {
    let ArcToParams {
        x1,
        y1,
        x2,
        y2,
        radius,
    } = *params;

    if radius <= 0.0 {
        path.line_to(x1, y1);
        return;
    }

    // Vectors from corner to adjacent points
    let v1x = x0 - x1;
    let v1y = y0 - y1;
    let v2x = x2 - x1;
    let v2y = y2 - y1;

    // Normalize vectors
    let len1 = (v1x * v1x + v1y * v1y).sqrt();
    let len2 = (v2x * v2x + v2y * v2y).sqrt();

    if len1 < 1e-6 || len2 < 1e-6 {
        path.line_to(x1, y1);
        return;
    }

    let v1x = v1x / len1;
    let v1y = v1y / len1;
    let v2x = v2x / len2;
    let v2y = v2y / len2;

    // Calculate angle between vectors
    let cross = v1x * v2y - v1y * v2x;
    let dot = v1x * v2x + v1y * v2y;
    let angle = cross.atan2(dot);

    if angle.abs() < 1e-6 {
        path.line_to(x1, y1);
        return;
    }

    // Calculate tangent points
    let tan_half = (angle / 2.0).tan().abs();
    let seg_len = radius / tan_half;

    let start_x = x1 + v1x * seg_len;
    let start_y = y1 + v1y * seg_len;
    let end_x = x1 + v2x * seg_len;
    let end_y = y1 + v2y * seg_len;

    // Calculate arc center
    let sign = if cross < 0.0 { -1.0 } else { 1.0 };
    let nx = -v1y * sign;
    let ny = v1x * sign;
    let cx = start_x + nx * radius;
    let cy = start_y + ny * radius;

    // Calculate start and end angles
    let start_angle = (start_y - cy).atan2(start_x - cx);
    let end_angle = (end_y - cy).atan2(end_x - cx);

    // Line to arc start
    path.line_to(start_x, start_y);

    // Draw arc - we just did line_to so we have a current point
    arc(
        path,
        &ArcParams {
            x: cx,
            y: cy,
            radius,
            start_angle,
            end_angle,
            anticlockwise: cross > 0.0,
        },
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::PathSegment;

    fn segments(builder: PathBuilder) -> Vec<PathSegment> {
        builder
            .finish()
            .map(|p| p.segments().collect())
            .unwrap_or_default()
    }

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn arc_params(
        x: f32,
        y: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        anticlockwise: bool,
    ) -> ArcParams {
        ArcParams {
            x,
            y,
            radius,
            start_angle,
            end_angle,
            anticlockwise,
        }
    }

    #[test]
    fn test_arc_full_circle() {
        let mut builder = PathBuilder::new();
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 50.0, 0.0, 2.0 * PI, false),
            false,
        );

        let segs = segments(builder);
        // First segment should be MoveTo at angle 0: (100, 50)
        assert!(
            matches!(segs[0], PathSegment::MoveTo(p) if approx_eq(p.x, 100.0) && approx_eq(p.y, 50.0))
        );
        // Full circle = 4 quarter-arc cubics
        let cubic_count = segs
            .iter()
            .filter(|s| matches!(s, PathSegment::CubicTo(..)))
            .count();
        assert_eq!(cubic_count, 4);
    }

    #[test]
    fn test_arc_quarter_circle() {
        let mut builder = PathBuilder::new();
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 50.0, 0.0, PI / 2.0, false),
            false,
        );

        let segs = segments(builder);
        // MoveTo at (100, 50), then one cubic
        assert!(
            matches!(segs[0], PathSegment::MoveTo(p) if approx_eq(p.x, 100.0) && approx_eq(p.y, 50.0))
        );
        assert_eq!(segs.len(), 2);
        // End point should be near (50, 100) — top of circle at angle π/2
        assert!(
            matches!(segs[1], PathSegment::CubicTo(_, _, end) if approx_eq(end.x, 50.0) && approx_eq(end.y, 100.0))
        );
    }

    #[test]
    fn test_arc_half_circle() {
        let mut builder = PathBuilder::new();
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 50.0, 0.0, PI, false),
            false,
        );

        let segs = segments(builder);
        // Half circle = 2 quarter-arc cubics
        let cubic_count = segs
            .iter()
            .filter(|s| matches!(s, PathSegment::CubicTo(..)))
            .count();
        assert_eq!(cubic_count, 2);
        // End point should be near (0, 50) — left side at angle π
        let last_cubic = segs
            .iter()
            .rev()
            .find(|s| matches!(s, PathSegment::CubicTo(..)))
            .unwrap();
        assert!(
            matches!(last_cubic, PathSegment::CubicTo(_, _, end) if approx_eq(end.x, 0.0) && approx_eq(end.y, 50.0))
        );
    }

    #[test]
    fn test_arc_without_current_point_starts_with_move() {
        let mut builder = PathBuilder::new();
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 30.0, 0.0, PI, false),
            false,
        );

        let segs = segments(builder);
        // No current point → first segment is MoveTo
        assert!(matches!(segs[0], PathSegment::MoveTo(..)));
        // Start point at angle 0: (80, 50)
        assert!(
            matches!(segs[0], PathSegment::MoveTo(p) if approx_eq(p.x, 80.0) && approx_eq(p.y, 50.0))
        );
    }

    #[test]
    fn test_arc_with_current_point_starts_with_line() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 30.0, 0.0, PI, false),
            true,
        );

        let segs = segments(builder);
        // Has current point → arc connects via LineTo to arc start (80, 50)
        assert!(
            matches!(segs[0], PathSegment::MoveTo(p) if approx_eq(p.x, 0.0) && approx_eq(p.y, 0.0))
        );
        assert!(
            matches!(segs[1], PathSegment::LineTo(p) if approx_eq(p.x, 80.0) && approx_eq(p.y, 50.0))
        );
    }

    #[test]
    fn test_arc_connects_to_existing_path() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.line_to(50.0, 50.0);
        // Arc centered at (80, 50) radius 30, starting at PI → start point (50, 50)
        arc(
            &mut builder,
            &arc_params(80.0, 50.0, 30.0, PI, 0.0, false),
            true,
        );

        let segs = segments(builder);
        // Should be one continuous path: MoveTo, LineTo(50,50), LineTo(~50,50), then cubics
        let move_count = segs
            .iter()
            .filter(|s| matches!(s, PathSegment::MoveTo(..)))
            .count();
        assert_eq!(move_count, 1, "should be one continuous subpath");
    }

    #[test]
    fn test_arc_anticlockwise() {
        let mut builder = PathBuilder::new();
        // Anticlockwise from 0 to π/2 sweeps the large way around (3π/2)
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 50.0, 0.0, PI / 2.0, true),
            false,
        );

        let segs = segments(builder);
        // Anticlockwise swaps start/end internally, so sweeps 3π/2 → 3 cubics
        let cubic_count = segs
            .iter()
            .filter(|s| matches!(s, PathSegment::CubicTo(..)))
            .count();
        assert_eq!(cubic_count, 3);
    }

    #[test]
    fn test_arc_bounds() {
        let mut builder = PathBuilder::new();
        arc(
            &mut builder,
            &arc_params(50.0, 50.0, 50.0, 0.0, 2.0 * PI, false),
            false,
        );

        let path = builder.finish().unwrap();
        let bounds = path.bounds();
        // Full circle centered at (50,50) radius 50 should span [0,100] x [0,100]
        assert!(bounds.left() < 1.0);
        assert!(bounds.top() < 1.0);
        assert!(bounds.right() > 99.0);
        assert!(bounds.bottom() > 99.0);
    }

    #[test]
    fn test_ellipse_different_radii() {
        let mut builder = PathBuilder::new();
        ellipse(
            &mut builder,
            &EllipseParams {
                x: 50.0,
                y: 50.0,
                radius_x: 80.0,
                radius_y: 30.0,
                rotation: 0.0,
                start_angle: 0.0,
                end_angle: 2.0 * PI,
                anticlockwise: false,
            },
            false,
        );

        let path = builder.finish().unwrap();
        let bounds = path.bounds();
        // Ellipse with rx=80, ry=30 centered at (50,50)
        // x: [50-80, 50+80] = [-30, 130], y: [50-30, 50+30] = [20, 80]
        assert!(bounds.left() < -29.0);
        assert!(bounds.right() > 129.0);
        assert!(bounds.top() < 21.0);
        assert!(bounds.bottom() > 79.0);
    }

    #[test]
    fn test_ellipse_zero_radius_is_noop() {
        let mut builder = PathBuilder::new();
        ellipse(
            &mut builder,
            &EllipseParams {
                x: 50.0,
                y: 50.0,
                radius_x: 0.0,
                radius_y: 30.0,
                rotation: 0.0,
                start_angle: 0.0,
                end_angle: PI,
                anticlockwise: false,
            },
            false,
        );
        // Zero radius_x → early return, path builder is empty
        assert!(builder.finish().is_none());
    }

    #[test]
    fn test_arc_to_geometry() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        // arcTo from (0,0) through corner (50,0) toward (50,50) with radius 20
        arc_to(
            &mut builder,
            0.0,
            0.0,
            &ArcToParams {
                x1: 50.0,
                y1: 0.0,
                x2: 50.0,
                y2: 50.0,
                radius: 20.0,
            },
        );

        let segs = segments(builder);
        // Should have: MoveTo(0,0), LineTo(tangent point on first edge), then cubics for arc
        assert!(matches!(segs[0], PathSegment::MoveTo(..)));
        // At least one LineTo (to tangent point) and at least one CubicTo (the arc)
        assert!(segs.iter().any(|s| matches!(s, PathSegment::LineTo(..))));
        assert!(segs.iter().any(|s| matches!(s, PathSegment::CubicTo(..))));
    }

    #[test]
    fn test_arc_to_zero_radius_lines_to_corner() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        arc_to(
            &mut builder,
            0.0,
            0.0,
            &ArcToParams {
                x1: 50.0,
                y1: 0.0,
                x2: 50.0,
                y2: 50.0,
                radius: 0.0,
            },
        );

        let segs = segments(builder);
        // Zero radius → just a line_to(x1, y1)
        assert_eq!(segs.len(), 2);
        assert!(
            matches!(segs[1], PathSegment::LineTo(p) if approx_eq(p.x, 50.0) && approx_eq(p.y, 0.0))
        );
    }
}
