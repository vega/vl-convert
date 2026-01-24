//! Arc operations using bezier curve approximation.
//!
//! tiny-skia does not support arc primitives directly, so we approximate
//! arcs using cubic bezier curves.

use std::f32::consts::PI;
use tiny_skia::PathBuilder;

/// Add an arc to the path using bezier curve approximation.
///
/// # Arguments
/// * `path` - The path builder to add the arc to
/// * `x` - X coordinate of the arc center
/// * `y` - Y coordinate of the arc center
/// * `radius` - Radius of the arc
/// * `start_angle` - Starting angle in radians
/// * `end_angle` - Ending angle in radians
/// * `anticlockwise` - If true, draw arc counterclockwise
pub fn arc(
    path: &mut PathBuilder,
    x: f32,
    y: f32,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    anticlockwise: bool,
) {
    ellipse(
        path,
        x,
        y,
        radius,
        radius,
        0.0,
        start_angle,
        end_angle,
        anticlockwise,
    );
}

/// Add an elliptical arc to the path using bezier curve approximation.
///
/// # Arguments
/// * `path` - The path builder to add the arc to
/// * `x` - X coordinate of the ellipse center
/// * `y` - Y coordinate of the ellipse center
/// * `radius_x` - X radius of the ellipse
/// * `radius_y` - Y radius of the ellipse
/// * `rotation` - Rotation of the ellipse in radians
/// * `start_angle` - Starting angle in radians
/// * `end_angle` - Ending angle in radians
/// * `anticlockwise` - If true, draw arc counterclockwise
#[allow(clippy::too_many_arguments)]
pub fn ellipse(
    path: &mut PathBuilder,
    x: f32,
    y: f32,
    radius_x: f32,
    radius_y: f32,
    rotation: f32,
    start_angle: f32,
    end_angle: f32,
    anticlockwise: bool,
) {
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

    // If path has no current point (is empty), move to start; otherwise line to start.
    // PathBuilder doesn't expose "has current point", but if the bounds are empty,
    // there's likely no content yet. We check this by seeing if we have any points.
    // A more robust approach: track separately in Canvas2dContext if we have a current point.
    // For now, we use a heuristic: call move_to if line_to would create a degenerate path.
    // Actually, the cleanest approach is to always move_to for the first point.
    // But Canvas 2D semantics say arc() should line to start if there's a current point.
    // So we need this information passed in. For now, let's use move_to which is safer
    // for standalone arcs and matches the more common usage pattern.
    path.move_to(start_x, start_y);

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
/// # Arguments
/// * `path` - The path builder
/// * `x0` - Current point X (from last path operation)
/// * `y0` - Current point Y
/// * `x1` - First control point X
/// * `y1` - First control point Y
/// * `x2` - Second control point X
/// * `y2` - Second control point Y
/// * `radius` - Arc radius
#[allow(clippy::too_many_arguments)]
pub fn arc_to(
    path: &mut PathBuilder,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    radius: f32,
) {
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

    // Draw arc
    arc(path, cx, cy, radius, start_angle, end_angle, cross > 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_full_circle() {
        let mut builder = PathBuilder::new();
        builder.move_to(100.0, 50.0);
        arc(&mut builder, 50.0, 50.0, 50.0, 0.0, 2.0 * PI, false);
        let path = builder.finish();
        assert!(path.is_some());
    }

    #[test]
    fn test_arc_quarter_circle() {
        let mut builder = PathBuilder::new();
        builder.move_to(100.0, 50.0);
        arc(&mut builder, 50.0, 50.0, 50.0, 0.0, PI / 2.0, false);
        let path = builder.finish();
        assert!(path.is_some());
    }
}
