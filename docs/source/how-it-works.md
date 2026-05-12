# How It Works

VlConvert is a Rust host for the official Vega and Vega-Lite JavaScript
libraries. It does not reimplement Vega's compiler, parser, scenegraph, or
renderer. Instead, `vl-convert-rs` embeds a JavaScript runtime, loads vendored
Vega modules into that runtime, and uses Rust code for the parts that need to
integrate with the host process: file and network access, font discovery,
Canvas 2D, SVG postprocessing, image encoding, PDF output, and API bindings.

The Python package, CLI, Rust crate, and HTTP server all use the same
`vl-convert-rs` converter.

## Runtime

Each converter has a worker pool. A worker is an operating-system thread with a
single-threaded Tokio runtime and a Deno `MainWorker`. Deno's `MainWorker`
wraps `deno_core::JsRuntime`, which embeds V8 through the Rusty V8 bindings.

VlConvert adds custom Deno extensions to that worker:

- JSON and MessagePack transfer ops for passing inputs and large results
  between Rust and JavaScript.
- Data-loading ops that Vega calls when a spec requests an HTTP, HTTPS, or file
  resource.
- A Canvas 2D extension used by Vega's canvas renderer.
- Bootstrap JavaScript that installs the Canvas polyfill and exposes the Rust
  ops needed by the generated conversion scripts.

The worker pool starts lazily on first use, or eagerly when callers invoke a
warm-up path. Requests are assigned to workers by current outstanding work so a
busy worker is skipped when another worker is available. The server uses the
same pool as the Rust and Python APIs.

## Startup Snapshot

`vl-convert-rs` builds a V8 startup snapshot at compile time. The snapshot
contains Deno runtime extensions plus VlConvert's runtime and Canvas 2D
extensions. At runtime, each worker starts from this snapshot rather than
constructing every extension from scratch.

The snapshot has two purposes:

- It reduces worker startup cost.
- It avoids runtime initialization paths that are fragile in packaged
  environments such as Python wheels and slim containers.

The JavaScript libraries themselves are still loaded as modules after worker
startup. The snapshot prepares the runtime environment that those modules run
inside.

## Vendored JavaScript

VlConvert vendors the JavaScript packages it needs: Vega, multiple supported
Vega-Lite versions, Vega Themes, Vega Embed, `lodash.debounce`, MessagePack,
D3 locale data, and their transitive dependencies.

The `vl-convert-vendor` helper downloads the modules with Deno, normalizes the
vendored tree, and generates a Rust import map. That generated map embeds each
module with `include_str!` and records the supported Vega-Lite versions.

At conversion time, the custom module loader resolves jsDelivr-style module
URLs to those embedded strings. Normal conversions therefore do not need network
access to load Vega or Vega-Lite. Network access is only involved when a spec,
font configuration, or plugin configuration asks for external resources.

## Conversion Pipeline

Vega-Lite compilation is performed by the official Vega-Lite JavaScript
compiler running inside the worker. A Vega-Lite input is compiled to a Vega
spec, and Vega then parses and evaluates the chart.

SVG output is produced by Vega's `view.toSVG()` path inside JavaScript.
Afterwards, Rust may postprocess the SVG to add font CSS, embed local or Google
Fonts, subset fonts, or inline image resources.

PNG output for Vega and Vega-Lite uses Vega's canvas renderer. VlConvert
provides a Canvas 2D polyfill in JavaScript and implements the drawing backend
in Rust. The rendered canvas is encoded as PNG bytes by Rust, including the
requested PPI metadata.

JPEG output is built from SVG output. Vega renders SVG first, then Rust
rasterizes the SVG and encodes JPEG bytes at the requested quality.

PDF output is also built from SVG output. Vega renders SVG first, then Rust
converts that SVG into PDF.

SVG input conversions skip Vega entirely. The SVG is parsed and rendered by
Rust libraries, then encoded as PNG, JPEG, or PDF depending on the requested
output.

Scenegraph output is produced in JavaScript from a Vega view. For the binary
mode, JavaScript serializes the scenegraph with MessagePack and Rust returns the
resulting bytes. For JSON output, Rust decodes the MessagePack payload into
JSON before returning it.

## Canvas 2D

Vega expects a browser-like or Node-like Canvas API when rendering to canvas.
VlConvert provides that API without Node canvas.

The `vl-convert-canvas2d-deno` crate installs JavaScript polyfills for
`HTMLCanvasElement`, `CanvasRenderingContext2D`, `Image`, `ImageData`,
`Path2D`, gradients, patterns, text metrics, and related APIs. Those polyfills
call Deno ops implemented in Rust.

The Rust drawing backend lives in `vl-convert-canvas2d`. It uses `tiny-skia`
for 2D drawing, `cosmic-text` for shaping and measuring text, `fontdb` for font
matching, and Rust image codecs for image decode and PNG output.

This is why Vega/Vega-Lite PNG output can run inside Deno/V8 without Node
canvas.

## Fonts

Fonts are part of the rendering pipeline, not only a presentation detail. Vega
needs accurate text measurement for layout, and raster output needs access to
glyph outlines.

VlConvert starts with a shared font baseline. The baseline includes vendored
Liberation Sans fonts, system fonts, and any registered font directories.
Workers clone that baseline into their local font databases. When the global
font configuration changes, workers refresh their font state before handling
the next conversion.

Google Fonts are handled as request-scoped overlays. When Google Fonts are
configured explicitly, or when automatic Google Fonts is enabled, VlConvert
resolves and downloads the needed families and variants, registers them with
the worker font database for that conversion, and clears the overlay after the
conversion finishes.

SVG postprocessing has a separate font step. Depending on options, VlConvert
can emit Google Fonts `@import` rules, embed local or downloaded fonts with
`@font-face`, subset embedded fonts to the characters used in the chart, and
inline external images.

Font usage and cache statistics are attached to conversion outputs. The server
uses this information for request budgeting and logs; library callers can use
it to understand which Google Fonts a spec actually used.

## Bundling

HTML output and `bundle-js` use a bundling path built into `vl-convert-rs`.
VlConvert constructs a Deno module graph, loads modules through its vendored
module loader, and emits a bundled JavaScript module with Deno AST/SWC tooling.
It does not shell out to the Deno executable at conversion time.

For Vega Embed bundles, the loader serves only vendored modules. For plugin
bundling, the loader can also fetch HTTP(S) imports when the plugin import
domain allowlist permits them. URL plugins, inline plugins, and file plugins
are bundled before they are loaded into a worker.

Bundled HTML can include Vega, Vega-Lite, Vega Embed, fonts, and plugins in the
generated document. Unbundled HTML references browser-loaded assets instead.

## Data and Access Control

Vega's loader is replaced with one that calls Rust ops for external data.
Those ops enforce the converter's `allowed_base_urls` policy before reading
HTTP(S) URLs or filesystem paths.

SVG image loading uses a related access policy when parsing or postprocessing
SVG. Plugin imports are controlled separately by plugin import-domain settings.

This separation matters because specs, images, fonts, and plugins are different
kinds of external resource access. Enabling one does not implicitly enable all
of the others.

## API Surfaces

`vl-convert-rs` is the core implementation. The other public surfaces adapt
inputs and outputs to their host environment:

- Python uses PyO3 bindings and exposes synchronous and asynchronous functions.
- The CLI maps subcommands and flags to Rust API calls.
- The server wraps a converter in Axum routes, middleware, request budgeting,
  admin configuration, and generated OpenAPI documentation.

Because all surfaces share the same Rust converter, most behavior differences
come from I/O shape rather than conversion semantics.

## Notable Crates

| Area | Crates | Role |
| ---- | ------ | ---- |
| JavaScript runtime | `deno_runtime`, `deno_core`, `v8` | Host Deno workers and V8 isolates inside Rust. |
| JavaScript bundling | `deno_graph`, `deno_ast` | Build module graphs and emit bundled JavaScript. |
| JS/Rust transfer | `serde_v8`, `rmp-serde` | Move structured values between V8, JSON, and MessagePack. |
| Canvas rendering | `tiny-skia`, `cosmic-text`, `fontdb` | Draw Canvas 2D output and shape text. |
| SVG parsing/rendering | `usvg`, `resvg` | Parse SVG and rasterize SVG input or SVG-derived outputs. |
| PDF output | `svg2pdf` | Convert static SVG output to PDF. |
| Image encoding | `png`, `image` | Encode PNGs, decode images, and encode JPEG output. |
| Font handling | `font-subset`, `vl-convert-google-fonts` | Subset embedded fonts and resolve/cache Google Fonts. |
| Python bindings | `pyo3`, `pyo3-async-runtimes` | Expose the Rust converter to Python. |
| Server | `axum`, `tower`, `utoipa` | Serve HTTP routes, middleware, and OpenAPI schemas. |
