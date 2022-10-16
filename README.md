# VlConvert
VlConvert provides a Rust library, CLI utility, and Python library for converting high-level [Vega-Lite](https://vega.github.io/vega-lite/) visualization specifications into low-level [Vega](https://vega.github.io/vega/) visualization specifications, all with no web browser or system dependencies required.

[:tada: Announcement Blog Post :tada:](https://medium.com/@jonmmease/introducing-vlconvert-c763f0076e89)

# Getting started
## CLI
Install `vl-convert` using [cargo](https://doc.rust-lang.org/cargo/) with:

```plain
$ cargo install vl-convert
$ vl-convert --help

vl-convert: A utility for converting Vega-Lite specifications

Usage: vl-convert <COMMAND>

Commands:
  vl2vg   Convert a Vega-Lite specification to a Vega specification
  vl2svg  Convert a Vega-Lite specification to an SVG image
  vl2png  Convert a Vega-Lite specification to an PNG image
  vg2svg  Convert a Vega specification to an SVG image
  vg2png  Convert a Vega specification to an PNG image
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into an SVG file named `out.svg`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library.

```plain
$ vl-convert vl2svg -i ./in.vl.json -o ./out.svg --vl-version 5.5
```

## Python
Install the `vl-convert-python` pacakge using pip

```
$ pip install vl-convert-python
```

UpdThen in Python, import the library, use the `vegalite_to_png` function to convert a Vega-Lite specification string to a PNG image, then write the image to a file.

```python
import vl_convert as vlc

vl_spec = r"""
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"url": "data/movies.json"},
  "mark": "circle",
  "encoding": {
    "x": {
      "bin": {"maxbins": 10},
      "field": "IMDB Rating"
    },
    "y": {
      "bin": {"maxbins": 10},
      "field": "Rotten Tomatoes Rating"
    },
    "size": {"aggregate": "count"}
  }
}
"""

png_data = vlc.vegalite_to_png(vl_spec=vl_spec, scale=2)
with open("chart.png", "wb") as f:
    f.write(png_data)
```

For more examples, see the [`vl-convert-python` README](https://github.com/jonmmease/vl-convert/tree/main/vl-convert-python#readme).

# Motivation
VlConvert was motivated by the needs of [VegaFusion](https://vegafusion.io/), which extracts data transformations from Vega specifications and evaluates them on the server. Using VlConvert, VegaFusion can input Vega-Lite specifications directly.  That said, VlConvert is designed to be used by the wider Vega-Lite ecosystem, independent of VegaFusion.

# How it works
VlConvert relies on the standard Vega-Lite JavaScript library to perform the Vega-Lite to Vega conversion.  It uses the [Deno](https://deno.land/) project (in particular [`deno_core`](https://github.com/denoland/deno/tree/main/core)) to run Vega-Lite using the v8 JavaScript runtime, embedded into a Rust library called `vl-convert-rs`. This Rust library is then wrapped as a CLI application (`vl-convert`) and a Python library using PyO3 (`vl-convert-python`).

In addition to easy embedding of the v8 JavaScript runtime, another advantage of building on `deno_core` is that it's possible to customize the module loading logic. `vl-convert-rs` takes advantage of this to inline the minified JavaScript source code for multiple versions of Vega-Lite, and all their dependencies, into the Rust library itself. This way, no internet connection is required to use vl-convert, and the executable and Python library are truly self-contained.

## Vega-Lite to Vega conversion
The Vega-Lite to Vega compilation is performed directly by the Vega-Lite library running fully in the Deno runtime.

## Vega(-Lite) to SVG
The Vega JavaScript library supports exporting chart specifications to SVG images, and this conversion works in Deno. However, there is a subtle complication. In order to properly position text within the exported SVG, Vega needs to compute the width of text fragments (at a particular font size, in a particular font, etc.). When running in Node.js, these calculations are done using node canvas, which does not work in Deno. When node canvas is not available, Vega falls back to a rough heuristic for text measurement that results in poor text placement results.

VlConvert works around this by overriding the text width calculation function using a custom Rust function. This custom Rust function uses the `usvg` crate (part of the `resvg` project) to compute the width of text fragments.  With this customization, we regain accurate text placement in the SVG results produced by Vega.

## Vega(-Lite) to PNG
The Vega JavaScript library supports exporting chart specifications directly to PNG images. When running in Node.js, this functionality relies on node canvas, which is not available in Deno.

VlConvert generates PNG images by first exporting charts to SVG as described above, then converting the SVG image to a PNG image using the `resvg` crate.
