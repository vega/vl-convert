//! Canvas comparison tests - compare vl-convert-canvas2d output against node-canvas.
//!
//! Each test renders the same drawing operations in both Rust and JavaScript (node-canvas),
//! then compares the resulting images using pixelmatch algorithm.
//!
//! ## Prerequisites
//!
//! These tests require node-canvas to be installed. Install it with:
//!
//! ```bash
//! cd vl-convert-canvas2d/tests/node_baseline
//! npm install
//! ```
//!
//! If node-canvas is not available, tests will be skipped.

mod common;

mod basic_drawing;
mod clipping;
mod fractional_coords;
mod gradients;
mod image_data;
mod image_smoothing;
mod max_width_text;
mod path2d_features;
mod patterns;
mod put_image_data;
mod transform_timing;
