//! DOMMatrix type for 2D transformation matrices.

/// DOMMatrix represents a 2D transformation matrix.
///
/// The matrix is represented as:
/// ```text
/// | a c e |
/// | b d f |
/// | 0 0 1 |
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DOMMatrix {
    /// Scale X component.
    pub a: f32,
    /// Skew Y component.
    pub b: f32,
    /// Skew X component.
    pub c: f32,
    /// Scale Y component.
    pub d: f32,
    /// Translate X component.
    pub e: f32,
    /// Translate Y component.
    pub f: f32,
}

impl DOMMatrix {
    /// Create a new DOMMatrix with the specified components.
    pub fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Create an identity matrix.
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }
}

impl From<tiny_skia::Transform> for DOMMatrix {
    fn from(t: tiny_skia::Transform) -> Self {
        DOMMatrix {
            a: t.sx,
            b: t.ky,
            c: t.kx,
            d: t.sy,
            e: t.tx,
            f: t.ty,
        }
    }
}

impl From<DOMMatrix> for tiny_skia::Transform {
    fn from(m: DOMMatrix) -> Self {
        tiny_skia::Transform::from_row(m.a, m.b, m.c, m.d, m.e, m.f)
    }
}
