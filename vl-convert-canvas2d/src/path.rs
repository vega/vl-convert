//! Path operations for Canvas 2D.
//!
//! This module provides path building operations that map to Canvas 2D API methods.

use tiny_skia::PathBuilder;

/// Extension trait for PathBuilder with Canvas 2D style methods.
pub trait PathBuilderExt {
    /// Add a rectangle to the path.
    fn rect(&mut self, x: f32, y: f32, width: f32, height: f32);
}

impl PathBuilderExt for PathBuilder {
    fn rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.move_to(x, y);
        self.line_to(x + width, y);
        self.line_to(x + width, y + height);
        self.line_to(x, y + height);
        self.close();
    }
}
