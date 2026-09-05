# How VlConvert Renders Charts

VlConvert runs the official Vega and Vega-Lite JavaScript libraries in an
embedded JavaScript runtime. The Python, CLI, Rust, and server interfaces share
this engine. Given the same configuration, fonts, and external resources, the
same input produces the same output from every interface.

## Input Types

| Input | Processing | Typical use |
| --- | --- | --- |
| Vega-Lite | Compile to Vega, then render | Charts authored in Vega-Lite or with Altair |
| Vega | Parse and render directly | Charts authored in Vega or already compiled from Vega-Lite |
| SVG | Convert with Rust image and PDF libraries | Existing SVG images that need PNG, JPEG, or PDF output |

VlConvert bundles several Vega-Lite versions. A conversion uses the version
selected by `vl_version` or `--vl-version`, so specifications written for an
older release still compile as their authors intended.

## Output Formats

- **SVG** keeps shapes and text as vectors. It is the best choice for web pages
  and later editing.
- **PDF** keeps vector output for documents and print.
- **PNG** is a lossless raster image. Use `scale` or `ppi` when the consumer
  needs more pixels.
- **JPEG** is a compressed raster image for cases where file size matters more
  than lossless output or transparency.
- **HTML** is an interactive page that renders the chart with Vega Embed in the
  reader's browser.
- **Vega, scenegraph, and font output** expose intermediate results: the
  compiled Vega specification, the evaluated scenegraph, and the fonts
  VlConvert resolves for a chart.
- **Vega Editor URL** encodes the specification in a link that opens it in the
  online Vega Editor.

PNG output uses Vega's canvas renderer with a Canvas 2D implementation supplied
by VlConvert. JPEG and PDF output start from the SVG rendering and are converted
by Rust image and PDF libraries. SVG input never runs Vega.

## Bundled JavaScript and Network Access

Vega, the supported Vega-Lite versions, Vega Themes, and Vega Embed are bundled
into VlConvert. A normal conversion downloads nothing.

A conversion uses the network only when the input or configuration asks for an
external resource:

- a specification with an HTTP data URL
- an image referenced by a specification or SVG file
- a Google Font that is not already cached
- a plugin loaded from a URL, or a plugin with HTTP imports

HTML output is a special case. Unless its dependencies are bundled into the
file, the reader's browser loads Vega from a content delivery network (CDN)
when the page opens.

Each resource type has its own access control. Allowing data URLs does not
allow plugin imports or Google Fonts. Read the security guide for your
interface before accepting specifications from untrusted callers.

## Fonts and Layout

Vega measures text while laying out a chart, so the fonts available at render
time affect label size, wrapping, and element positions. VlConvert bundles
Liberation Sans, loads the fonts installed on the host, and can also use
registered font directories and explicitly enabled Google Fonts. SVG and HTML
output can embed fonts so another machine displays the same typeface. See the
fonts guide for your interface when text looks different from a browser
preview.

## Conversion Workers

The Python, Rust, and server interfaces keep a pool of workers. Each worker
owns one JavaScript runtime and handles one conversion at a time. Reusing a
converter avoids creating a runtime for every chart, and a pool with several
workers runs conversions concurrently.

Workers start on first use. Long-lived applications can warm them at startup
when first-request latency matters. The CLI creates a single-worker converter
for each command, so these controls do not apply to it.

## Interfaces

- **Python** returns strings, dictionaries, or bytes.
- **CLI** reads files or standard input and writes files or standard output.
- **Rust** returns output structs that carry the result and Vega's diagnostic
  messages.
- **Server** exposes conversion over HTTP and adds authentication, request
  limits, logging, and optional live administration.

All four are built on the `vl-convert-rs` crate, which embeds the JavaScript
runtime and implements data loading, font discovery, image encoding, and PDF
output.
