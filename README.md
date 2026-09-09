# VlConvert

VlConvert converts [Vega-Lite](https://vega.github.io/vega-lite/),
[Vega](https://vega.github.io/vega/), and SVG input into formats for documents,
applications, and web pages. It is available as:

- the `vl-convert-rs` Rust library
- the `vl-convert` command-line interface and HTTP server
- the `vl-convert-python` Python package

For Vega and Vega-Lite charts, VlConvert produces SVG, PNG, JPEG, PDF, HTML,
scenegraphs, font metadata, and Vega Editor URLs. It also compiles Vega-Lite to
Vega and converts existing SVG to PNG, JPEG, or PDF. VlConvert embeds the
official Vega and Vega-Lite JavaScript libraries, so conversions do not require
a browser or Node.js.

## Quick Start

### Command Line

Install the CLI from crates.io:

```bash
cargo install vl-convert --locked
```

Convert a Vega-Lite specification to SVG:

Save this input as `chart.vl.json`:

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {"values": [{"category": "A", "value": 3}, {"category": "B", "value": 7}]},
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "value", "type": "quantitative"}
  }
}
```

```bash
vl-convert vl2svg --input chart.vl.json --output chart.svg
```

Input and output default to standard input and standard output. Global options
such as resource permissions and logging go before the conversion command.

### Python

Add the Python package to a project with
[uv](https://docs.astral.sh/uv/):

```bash
uv add "vl-convert-python>=2.0.0rc1,<3"
```

Render a specification with inline data:

```python
import vl_convert as vlc

spec = {
    "data": {"values": [{"category": "A", "value": 3}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "category", "type": "nominal"},
        "y": {"field": "value", "type": "quantitative"},
    },
}

png = vlc.vegalite_to_png(spec, scale=2)
with open("chart.png", "wb") as output_file:
    output_file.write(png)
```

### Rust

Add the library to a Rust project:

```toml
[dependencies]
vl-convert-rs = "2"
```

`VlConverter` provides asynchronous methods for Vega, Vega-Lite, and SVG
conversions. See the [`vl-convert-rs` API](https://docs.rs/vl-convert-rs/) for
the Rust types and examples.

### HTTP Server

The CLI package includes a long-running conversion server:

```bash
vl-convert serve --port 3000
```

The server exposes interactive API documentation at
`http://127.0.0.1:3000/docs`. Configure authentication, resource access, and
runtime limits before exposing it outside a trusted machine or network.

## Network and File Access

The bundled Vega libraries need no network access. A conversion can still load
remote data, images, Google Fonts, or plugins when the input and configuration
request them. Relative sample-data paths such as `data/cars.json` use the
`vega-datasets` CDN by default.

Data and images share the `allowed_base_urls` access policy. Local files are
blocked by default. Google Fonts and plugins use separate controls. See the
[data-loading guide](docs/source/_topics/guides/data-loading.md) and
[security guide](docs/source/_topics/guides/security.md) before processing
specifications from untrusted sources.

## Rendering Model

VlConvert runs the official Vega-Lite compiler and Vega renderer in an embedded
V8 runtime. Vega and Vega-Lite PNG output uses Vega's canvas renderer through
VlConvert's Canvas 2D implementation. JPEG and PDF conversions start from SVG
output. SVG input conversions use Rust image and PDF libraries without running
Vega.

The [documentation source](docs/source/index.md) covers installation,
configuration, each interface, output formats, fonts, plugins, and server
operation.

## Development

This repository uses [Pixi](https://pixi.sh/) for its development environment.
Common checks include:

```bash
pixi run test-rs
pixi run test-cli
pixi run test-py
pixi run docs-build-strict
```

VlConvert is distributed under the BSD 3-Clause license.
