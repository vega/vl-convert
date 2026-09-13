# Architecture

VlConvert runs Vega's JavaScript libraries inside Rust. Vega-Lite compiles specifications to Vega, Vega evaluates and lays out the chart, and Rust handles drawing and export. The Python package, CLI, and HTTP server all wrap the `vl-convert-rs` crate.

## Embedded JavaScript

Vega libraries run in an embedded Deno JavaScript runtime, built on V8. Deno connects JavaScript to Rust through *ops*: Rust functions exposed to JavaScript. VlConvert uses these to transfer specifications and results, load data, and draw on the canvas.

A custom Canvas 2D implementation is used, which has two layers. `vl-convert-canvas2d-deno` supplies the JavaScript objects Vega expects, including drawing contexts, images, paths, and text metrics. It passes drawing calls to `vl-convert-canvas2d`, which implements them in Rust. No browser process is involved.

Vega, the supported Vega-Lite versions, Vega Embed, themes, and locale data are embedded separately as JavaScript modules. The `vl-convert-vendor` tool downloads dependencies during updates and generates their module map. The runtime loads these modules from the binary, without requiring an internet connection.

## Google Fonts

The `vl-convert-google-fonts` crate uses Google's CSS2 API to resolve font families, weights, and styles to downloadable font files. The client supports both async and blocking callers without depending on Deno or V8.

Downloaded fonts are registered through `fontdb` for the conversions that need them, not installed as system fonts. Chart conversions temporarily add them to the worker's font database.

## Rust Crates

The following is a table of notable crates in the Rust ecosystem that power VlConvert.

| Area | Crates | Role |
| --- | --- | --- |
| JavaScript runtime | `deno_runtime`, `deno_core`, `v8` | Embed Deno and V8, expose Rust ops, and create runtime snapshots |
| Async execution | `tokio` | Run worker event loops, network requests, and blocking SVG conversions |
| JavaScript bundling | `deno_graph`, `deno_ast` | Build module graphs and bundle JavaScript using SWC |
| Value transfer | `serde_v8`, `serde_json`, `rmp-serde` | Convert between Rust and V8 values and encode JSON or MessagePack |
| Canvas drawing | `tiny-skia`, `cosmic-text`, `fontdb` | Draw shapes, shape and measure text, and match fonts |
| SVG and PDF | `usvg`, `resvg`, `svg2pdf` | Parse SVG, rasterize it, or convert it to PDF |
| Images | `png`, `image` | Encode PNG, decode image data, and encode JPEG |
| Fonts | `ttf-parser`, `font-subset`, `vl-convert-google-fonts` | Inspect and subset font files, and download and cache Google Fonts |
| Python bindings | `pyo3`, `pyo3-async-runtimes` | Expose synchronous and asynchronous Python functions |
| HTTP server | `axum`, `tower`, `tower-http`, `utoipa` | Provide routing, middleware, and OpenAPI schemas |
