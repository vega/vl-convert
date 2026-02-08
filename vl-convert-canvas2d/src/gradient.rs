//! Gradient types for Canvas 2D operations.

use crate::geometry::RadialGradientParams;

/// A color stop in a gradient.
#[derive(Debug, Clone)]
pub struct GradientStop {
    /// Offset position (0.0 to 1.0).
    pub offset: f64,
    /// Color at this stop.
    pub color: tiny_skia::Color,
}

/// Canvas gradient (linear or radial).
#[derive(Debug, Clone)]
pub struct CanvasGradient {
    /// Gradient type and geometry.
    pub gradient_type: GradientType,
    /// Color stops.
    pub stops: Vec<GradientStop>,
}

/// Type of gradient.
#[derive(Debug, Clone)]
pub enum GradientType {
    /// Linear gradient from (x0, y0) to (x1, y1).
    Linear { x0: f32, y0: f32, x1: f32, y1: f32 },
    /// Radial gradient from inner circle to outer circle.
    Radial(RadialGradientParams),
}

impl CanvasGradient {
    /// Create a new linear gradient.
    pub fn new_linear(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            gradient_type: GradientType::Linear { x0, y0, x1, y1 },
            stops: Vec::new(),
        }
    }

    /// Create a new radial gradient.
    pub fn new_radial(params: &RadialGradientParams) -> Self {
        Self {
            gradient_type: GradientType::Radial(*params),
            stops: Vec::new(),
        }
    }

    /// Add a color stop to the gradient.
    pub fn add_color_stop(&mut self, offset: f64, color: tiny_skia::Color) {
        self.stops.push(GradientStop { offset, color });
        // Keep stops sorted by offset
        self.stops.sort_by(|a, b| {
            a.offset
                .partial_cmp(&b.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}
