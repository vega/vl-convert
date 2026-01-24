//! Error types for vl-convert-canvas2d.

use thiserror::Error;

/// Result type alias using Canvas2dError.
pub type Canvas2dResult<T> = Result<T, Canvas2dError>;

/// Errors that can occur in Canvas 2D operations.
#[derive(Debug, Error)]
pub enum Canvas2dError {
    /// Invalid canvas dimensions (must be positive and within limits).
    #[error("Invalid dimensions: width={width}, height={height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// Failed to parse CSS font string.
    #[error("Failed to parse font string: {0}")]
    FontParseError(String),

    /// Failed to parse color value.
    #[error("Failed to parse color: {0}")]
    ColorParseError(String),

    /// PNG encoding error.
    #[error("PNG encoding error: {0}")]
    PngError(String),

    /// Invalid gradient stop offset (must be 0.0-1.0).
    #[error("Invalid gradient stop offset: {0} (must be 0.0-1.0)")]
    InvalidGradientStop(f64),

    /// Path operation error.
    #[error("Path error: {0}")]
    PathError(String),

    /// Text rendering error.
    #[error("Text rendering error: {0}")]
    TextError(String),
}

impl From<png::EncodingError> for Canvas2dError {
    fn from(err: png::EncodingError) -> Self {
        Canvas2dError::PngError(err.to_string())
    }
}
