//! Path2D implementation for reusable path objects.
//!
//! Path2D allows creating path objects that can be reused across multiple
//! fill, stroke, or clip operations.

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
        }
    }

    /// Create a copy of another Path2D.
    pub fn from_path(other: &Path2D) -> Self {
        other.clone()
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
    }

    /// Draw a line to a point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.invalidate();
        self.builder.line_to(x, y);
        self.current_x = x;
        self.current_y = y;
    }

    /// Close the current subpath.
    pub fn close_path(&mut self) {
        self.invalidate();
        self.builder.close();
        self.current_x = self.subpath_start_x;
        self.current_y = self.subpath_start_y;
    }

    /// Add a cubic bezier curve.
    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.invalidate();
        self.builder.cubic_to(cp1x, cp1y, cp2x, cp2y, x, y);
        self.current_x = x;
        self.current_y = y;
    }

    /// Add a quadratic bezier curve.
    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.invalidate();
        self.builder.quad_to(cpx, cpy, x, y);
        self.current_x = x;
        self.current_y = y;
    }

    /// Add a rectangle to the path.
    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.invalidate();
        self.move_to(x, y);
        self.line_to(x + width, y);
        self.line_to(x + width, y + height);
        self.line_to(x, y + height);
        self.close_path();
    }

    /// Add a rounded rectangle to the path with uniform corner radius.
    pub fn round_rect(&mut self, x: f32, y: f32, width: f32, height: f32, radius: f32) {
        self.round_rect_radii(x, y, width, height, [radius, radius, radius, radius]);
    }

    /// Add a rounded rectangle to the path with individual corner radii.
    pub fn round_rect_radii(&mut self, x: f32, y: f32, width: f32, height: f32, radii: [f32; 4]) {
        self.invalidate();

        // Handle negative dimensions
        let (x, width) = if width < 0.0 {
            (x + width, -width)
        } else {
            (x, width)
        };
        let (y, height) = if height < 0.0 {
            (y + height, -height)
        } else {
            (y, height)
        };

        let [mut tl, mut tr, mut br, mut bl] = radii;

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
    pub fn arc(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        anticlockwise: bool,
    ) {
        self.invalidate();
        crate::arc::arc(
            &mut self.builder,
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
        self.invalidate();
        crate::arc::arc_to(
            &mut self.builder,
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
        self.invalidate();
        crate::arc::ellipse(
            &mut self.builder,
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

    #[test]
    fn test_path2d_new() {
        let path = Path2D::new();
        assert!(path.path.is_none());
    }

    #[test]
    fn test_path2d_rect() {
        let mut path = Path2D::new();
        path.rect(10.0, 10.0, 50.0, 50.0);
        assert!(path.get_path().is_some());
    }

    #[test]
    fn test_path2d_clone() {
        let mut path1 = Path2D::new();
        path1.rect(10.0, 10.0, 50.0, 50.0);
        let path2 = Path2D::from_path(&path1);
        assert!(path2.path.is_none()); // Clone doesn't copy cached path
    }
}
