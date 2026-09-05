# Architecture

VlConvert is a Rust host for the official Vega and Vega-Lite JavaScript
libraries. It does not reimplement Vega's compiler, parser, scenegraph, or
renderer. The `vl-convert-rs` crate embeds a JavaScript runtime, loads the
bundled Vega libraries into it, and supplies Rust code for everything that has
to touch the host machine: file and network access, fonts, Canvas 2D drawing,
SVG postprocessing, image encoding, PDF output, and the language bindings.

The Python package, CLI, Rust crate, and HTTP server all drive this one
converter. {doc}`rendering` describes what that means for users. This page
describes how the pieces fit together.

## Runtime

Each converter owns a pool of workers. A worker is a thread with its own Deno
runtime, and Deno is a JavaScript runtime built on V8, the JavaScript engine
from Chrome. Vega runs inside the worker, and it calls back into Rust through
ops, which are Rust functions that Deno exposes to JavaScript. VlConvert's ops
move specifications and results between the two languages, load data on
Vega's behalf, and implement Canvas 2D drawing.

Requests go to the least busy worker, and each worker handles one conversion
at a time. Per-request plugins run on separate short-lived workers so that
caller-supplied code never touches the shared pool.

Two build-time steps keep startup fast and self-contained. A V8 snapshot holds
the Deno runtime together with VlConvert's extensions, so a worker starts from
a prepared state instead of building it up. And the JavaScript that VlConvert
needs is embedded in the binary: Vega, every supported Vega-Lite version, Vega
Themes, Vega Embed, D3 locale data, and their dependencies. The
`vl-convert-vendor` tool downloads those packages and generates the module map
that the runtime loads them from, so loading Vega never uses the network. Only
a specification, font setting, or plugin setting that asks for an external
resource does.

## Conversion Pipeline

Vega-Lite compilation runs the official Vega-Lite compiler inside the worker,
and Vega then parses and evaluates the compiled specification.

- **SVG** comes from Vega's `view.toSVG()`. Rust may then postprocess the
  document to add font CSS, embed fonts, subset them, or inline images.
- **PNG** for Vega and Vega-Lite input uses Vega's canvas renderer drawing on
  VlConvert's Canvas 2D implementation. Rust encodes the pixels as PNG with the
  requested pixel-density metadata.
- **JPEG** and **PDF** start from the SVG output. Rust rasterizes it for JPEG at
  the requested quality, or converts it to PDF.
- **SVG input** never runs Vega. Rust parses and renders the document, then
  encodes PNG, JPEG, or PDF.
- **Scenegraph** output is produced in JavaScript from the Vega view, as JSON
  or as MessagePack bytes.

## Canvas 2D

Vega's canvas renderer expects the browser's Canvas API. VlConvert provides it
without Node canvas. The `vl-convert-canvas2d-deno` crate installs JavaScript
polyfills for the canvas objects Vega uses, such as the drawing context,
images, paths, gradients, and text metrics, and each polyfill calls into Rust.
The drawing backend is the `vl-convert-canvas2d` crate, which uses `tiny-skia`
to draw, `cosmic-text` to shape and measure text, and `fontdb` to match fonts.

Images drawn on the canvas load through the same data-loading ops as chart
data, so they follow the same access policy.

## Fonts

Fonts affect layout, not only appearance. Vega measures text to lay out
labels, axes, legends, and titles, and raster output needs the glyph outlines,
so the fonts available when a chart is rendered change the result.

Workers draw on one shared set of fonts: the bundled Liberation Sans fonts, the
fonts installed on the host, and any registered font directories. Google Fonts
are added per conversion. VlConvert resolves the variants a chart needs,
downloads them through an on-disk cache, and makes them available for that
conversion only.

SVG output has a font step of its own. Depending on options, postprocessing
emits Google Fonts `@import` rules, embeds fonts as `@font-face`, subsets them
to the characters used, and inlines external images. Every output also carries
statistics about the Google Fonts it used, which the server feeds into request
budgets and logs.

## Bundling

HTML output and `bundle-js` use a bundler built into `vl-convert-rs`. It
assembles a module graph from the vendored modules and emits a single
JavaScript file using Deno's own bundling libraries, without running a Deno
executable. Plugins are bundled the same way before a worker loads them, and
their HTTP imports are allowed only when they match the plugin import
allowlist. Bundled HTML inlines Vega, Vega-Lite, Vega Embed, plugins, and the
fonts VlConvert resolved for the chart. Unbundled HTML loads the libraries from
a CDN in the browser.

## Data and Access Control

Vega's loader is replaced with one that calls the data-loading ops, and the
canvas image polyfill and the SVG image resolver use the same rules. Every HTTP
URL and filesystem path is checked against `allowed_base_urls`, including the
destination of each redirect, before it is read. A filesystem `base_url` only
resolves relative references and grants no access on its own. The result is a
single policy for a chart whether it renders on the canvas or through SVG.

Plugin imports are governed separately by the plugin import allowlists, and
Google Fonts downloads by the font settings. That separation is deliberate:
data, images, fonts, and plugins are different kinds of external access, and
enabling one never enables another.

## API Surfaces

`vl-convert-rs` is the implementation. The other surfaces adapt inputs and
outputs to their hosts:

- The Python package uses PyO3, the Rust bindings for Python, and offers both
  synchronous and asynchronous functions.
- The CLI maps subcommands and flags onto the Rust API.
- The server wraps a converter in Axum routes and Tower middleware, adds request
  budgets and admin configuration, and generates its OpenAPI document with
  utoipa.

Because every surface shares the converter, they differ in how input and output
are shaped, not in how charts are converted. The server does not cache rendered
output; put an HTTP or application cache in front of it when identical
specifications are rendered repeatedly.

## Notable Crates

| Area | Crates | Role |
| --- | --- | --- |
| JavaScript runtime | `deno_runtime`, `deno_core`, `v8` | Host Deno workers and V8 isolates inside Rust |
| JavaScript bundling | `deno_graph`, `deno_ast` | Build module graphs and emit bundled JavaScript |
| JS/Rust transfer | `serde_v8`, `rmp-serde` | Move values between V8, JSON, and MessagePack |
| Canvas rendering | `tiny-skia`, `cosmic-text`, `fontdb` | Draw Canvas 2D output and shape text |
| SVG parsing and rendering | `usvg`, `resvg` | Parse SVG and rasterize SVG input or SVG-derived outputs |
| PDF output | `svg2pdf` | Convert SVG output to PDF |
| Image encoding | `png`, `image` | Encode PNG, decode images, and encode JPEG |
| Font handling | `font-subset`, `vl-convert-google-fonts` | Subset embedded fonts and resolve and cache Google Fonts |
| Python bindings | `pyo3`, `pyo3-async-runtimes` | Expose the converter to Python |
| Server | `axum`, `tower`, `tower-http`, `utoipa` | Serve HTTP routes, middleware, and OpenAPI schemas |
