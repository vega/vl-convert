//! Transform operations for Canvas2dContext.

use super::Canvas2dContext;
use crate::dom_matrix::DOMMatrix;
use tiny_skia::Transform;

impl Canvas2dContext {
    /// Translate the canvas.
    pub fn translate(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "translate {} {}", x, y);
        self.state.transform = self.state.transform.pre_translate(x, y);
    }

    /// Rotate the canvas.
    pub fn rotate(&mut self, angle: f32) {
        log::debug!(target: "canvas", "rotate {}", angle);
        let cos = angle.cos();
        let sin = angle.sin();
        let rotation = Transform::from_row(cos, sin, -sin, cos, 0.0, 0.0);
        self.state.transform = self.state.transform.pre_concat(rotation);
    }

    /// Scale the canvas.
    pub fn scale(&mut self, x: f32, y: f32) {
        log::debug!(target: "canvas", "scale {} {}", x, y);
        self.state.transform = self.state.transform.pre_scale(x, y);
    }

    /// Apply a transform matrix.
    pub fn transform(&mut self, matrix: DOMMatrix) {
        log::debug!(target: "canvas", "transform {:?}", matrix);
        let t: Transform = matrix.into();
        self.state.transform = self.state.transform.pre_concat(t);
    }

    /// Set the transform matrix (replacing the current one).
    pub fn set_transform(&mut self, matrix: DOMMatrix) {
        log::debug!(target: "canvas", "setTransform {:?}", matrix);
        self.state.transform = matrix.into();
    }

    /// Reset the transform to identity.
    pub fn reset_transform(&mut self) {
        log::debug!(target: "canvas", "resetTransform");
        self.state.transform = Transform::identity();
    }

    /// Get the current transformation matrix.
    pub fn get_transform(&self) -> DOMMatrix {
        self.state.transform.into()
    }
}
