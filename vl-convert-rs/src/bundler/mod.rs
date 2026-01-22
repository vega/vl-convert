//! JavaScript bundler for vl-convert.
//!
//! This module provides JavaScript bundling functionality for vl-convert's HTML export feature.
//! It absorbs essential functionality from the deprecated deno_emit crate, adapted to work
//! with current versions of deno_graph and deno_ast.
//!
//! The bundler creates a self-contained JavaScript file by:
//! 1. Building a module graph from vendored Vega/Vega-Lite libraries
//! 2. Transpiling TypeScript/JSX if needed
//! 3. Bundling all modules into a single minified ES module
//!
//! # Architecture
//!
//! - `loader`: Implements `deno_graph::source::Loader` to serve vendored modules
//! - `bundle_hook`: Handles `import.meta` rewriting during bundling
//! - `emit`: Core bundling logic using SWC bundler
//! - `text`: Utility functions for text processing

mod bundle_hook;
mod emit;
mod loader;
mod text;

pub use emit::{bundle, BundleEmit, BundleOptions, BundleType};
