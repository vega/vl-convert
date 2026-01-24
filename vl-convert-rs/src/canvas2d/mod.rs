//! Canvas 2D ops module for Deno integration.
//!
//! This module provides Deno ops that wrap the vl-convert-canvas2d crate,
//! enabling JavaScript code to use Canvas 2D API via Rust.

mod ops;
mod resource;

pub use ops::*;
pub use resource::CanvasResource;
