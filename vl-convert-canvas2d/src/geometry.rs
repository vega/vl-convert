//! Parameter structs for Canvas 2D drawing operations.
//!
//! These structs replace long positional argument lists with named fields,
//! grouping semantically related parameters together.

/// Parameters for a circular arc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcParams {
    /// X coordinate of the arc center.
    pub x: f32,
    /// Y coordinate of the arc center.
    pub y: f32,
    /// Radius of the arc.
    pub radius: f32,
    /// Starting angle in radians.
    pub start_angle: f32,
    /// Ending angle in radians.
    pub end_angle: f32,
    /// If true, draw arc counterclockwise.
    pub anticlockwise: bool,
}

/// Parameters for an elliptical arc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EllipseParams {
    /// X coordinate of the ellipse center.
    pub x: f32,
    /// Y coordinate of the ellipse center.
    pub y: f32,
    /// X-axis radius of the ellipse.
    pub radius_x: f32,
    /// Y-axis radius of the ellipse.
    pub radius_y: f32,
    /// Rotation of the ellipse in radians.
    pub rotation: f32,
    /// Starting angle in radians.
    pub start_angle: f32,
    /// Ending angle in radians.
    pub end_angle: f32,
    /// If true, draw arc counterclockwise.
    pub anticlockwise: bool,
}

impl From<&ArcParams> for EllipseParams {
    fn from(arc: &ArcParams) -> Self {
        Self {
            x: arc.x,
            y: arc.y,
            radius_x: arc.radius,
            radius_y: arc.radius,
            rotation: 0.0,
            start_angle: arc.start_angle,
            end_angle: arc.end_angle,
            anticlockwise: arc.anticlockwise,
        }
    }
}

/// Parameters for an arcTo operation.
///
/// The arc is drawn from the current point through a tangent defined by
/// two control points with a given radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcToParams {
    /// First control point X.
    pub x1: f32,
    /// First control point Y.
    pub y1: f32,
    /// Second control point X.
    pub x2: f32,
    /// Second control point Y.
    pub y2: f32,
    /// Arc radius.
    pub radius: f32,
}

/// Parameters for a cubic Bezier curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezierParams {
    /// First control point X.
    pub cp1x: f32,
    /// First control point Y.
    pub cp1y: f32,
    /// Second control point X.
    pub cp2x: f32,
    /// Second control point Y.
    pub cp2y: f32,
    /// End point X.
    pub x: f32,
    /// End point Y.
    pub y: f32,
}

/// Source and destination rectangles for a cropped drawImage operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageCropParams {
    /// Source rectangle X.
    pub sx: f32,
    /// Source rectangle Y.
    pub sy: f32,
    /// Source rectangle width.
    pub sw: f32,
    /// Source rectangle height.
    pub sh: f32,
    /// Destination rectangle X.
    pub dx: f32,
    /// Destination rectangle Y.
    pub dy: f32,
    /// Destination rectangle width.
    pub dw: f32,
    /// Destination rectangle height.
    pub dh: f32,
}

/// A dirty rectangle for partial image data writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtyRect {
    /// X offset into the source data.
    pub x: i32,
    /// Y offset into the source data.
    pub y: i32,
    /// Width of region to copy.
    pub width: i32,
    /// Height of region to copy.
    pub height: i32,
}

/// Parameters for creating a radial gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialGradientParams {
    /// Inner circle center X.
    pub x0: f32,
    /// Inner circle center Y.
    pub y0: f32,
    /// Inner circle radius.
    pub r0: f32,
    /// Outer circle center X.
    pub x1: f32,
    /// Outer circle center Y.
    pub y1: f32,
    /// Outer circle radius.
    pub r1: f32,
}
