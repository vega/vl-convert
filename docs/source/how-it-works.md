# How VlConvert Renders Charts

VlConvert runs the official Vega and Vega-Lite JavaScript libraries in an
embedded runtime. The Rust, Python, CLI, and server interfaces all use the same
conversion engine, so they produce the same output for the same input and
options.

## Choose the Input Type

VlConvert accepts three input types:

| Input | Processing | Typical Use |
| --- | --- | --- |
| Vega-Lite | Compile to Vega, then render | Most charts authored with Vega-Lite or Altair |
| Vega | Parse and render directly | Charts already compiled to Vega or authored with Vega |
| SVG | Convert with Rust image libraries | Existing SVG images that need PNG, JPEG, or PDF output |

Vega-Lite compilation uses the Vega-Lite version selected by `vl_version` or
`--vl-version`. VlConvert includes several Vega-Lite versions so applications
can render specifications created by different releases.

## Choose the Output Format

- **SVG** preserves vector shapes and text. It is usually the best choice for
  web pages, print workflows, and later editing.
- **PNG** produces a lossless raster image. Use `scale` or `ppi` when the
  consumer needs more pixels.
- **JPEG** produces a compressed raster image. It is useful when file size
  matters more than lossless output or transparency.
- **PDF** preserves vector output for documents and print workflows.
- **HTML** creates an interactive Vega Embed document. It can reference
  browser-loaded dependencies or include them in the file.
- **Vega and scenegraph output** expose intermediate representations for tools
  that need to inspect or transform the evaluated chart.

Vega and Vega-Lite PNG output uses Vega's canvas renderer. VlConvert supplies
the Canvas 2D implementation and encodes the result as PNG. JPEG and PDF output
start from SVG and are converted by Rust image and PDF libraries. SVG input
conversions do not start Vega or Vega-Lite.

See the image quality guide for your interface for format and resolution
options.

## Bundled JavaScript and Network Access

VlConvert includes Vega, supported Vega-Lite versions, Vega Themes, and Vega
Embed. Normal conversion does not download these libraries at runtime.

A conversion can still use the network when the input or configuration asks
for an external resource. Common examples include:

- a Vega or Vega-Lite specification with an HTTP data URL
- an image referenced by a specification or SVG file
- a Google Font that is not already cached
- a plugin loaded from a URL or a plugin with HTTP imports
- an HTML file configured to load browser dependencies from a content delivery
  network (CDN)

These resource types have separate access controls. Allowing a data URL does
not also allow plugin imports or Google Fonts. Review the security guide for
your interface before accepting specifications from untrusted callers.

## Fonts and Layout

Vega measures text while it lays out a chart. The selected fonts therefore
affect label size, line wrapping, and the final positions of chart elements.

VlConvert can use its included Liberation Sans fonts, system fonts, registered
font directories, and explicitly enabled Google Fonts. SVG and HTML output can
also embed fonts so another machine can display the intended typeface. See the
font guide for your interface when a chart has missing fonts or text that does
not align as expected.

## Conversion Workers

The Python, Rust, and server interfaces keep a pool of conversion workers. A
worker is an execution context that owns one JavaScript runtime. Reusing a
converter avoids creating that runtime for every chart and permits concurrent
work when the pool contains multiple workers.

Workers start when they are first needed. Long-lived applications can warm
them during startup when first-request latency matters. The CLI creates a
converter for each command, so most CLI users do not need worker lifecycle or
warm-up controls.

## Public Interfaces

- **Python** returns strings, dictionaries, or bytes that an application can
  save or pass to another library.
- **CLI** reads files or standard input and writes files or standard output.
- **Rust** returns output structs that include the rendered result and
  conversion diagnostics.
- **Server** exposes conversion over HTTP and adds authentication, request
  limits, logging, and optional live administration.

The core implementation is the `vl-convert-rs` crate. It embeds the JavaScript
runtime and handles host operations such as data loading, font discovery,
image encoding, and PDF output. Most users can select an interface without
depending on those implementation details.
