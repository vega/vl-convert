# VlConvert
VlConvert provides a Rust library, CLI utility, and Python library for converting [Vega-Lite](https://vega.github.io/vega-lite/) chart specifications into static images (SVG or PNG) or [Vega](https://vega.github.io/vega/) chart specifications.

[:tada: Announcement Blog Post :tada:](https://medium.com/@jonmmease/introducing-vlconvert-c763f0076e89)

Try the Python library out on Binder! \
[![Binder](https://mybinder.org/badge_logo.svg)](https://mybinder.org/v2/gh/jonmmease/vl-convert/main?labpath=vl-convert-python%2Fnotebooks%2Fconvert_vegalite.ipynb)


# Getting started
## CLI
Install `vl-convert` using [cargo](https://doc.rust-lang.org/cargo/) with:

```plain
$ cargo install vl-convert --locked
$ vl-convert --help

vl-convert: A utility for converting Vega-Lite specifications

Usage: vl-convert <COMMAND>

Commands:
  vl2vg      Convert a Vega-Lite specification to a Vega specification
  vl2svg     Convert a Vega-Lite specification to an SVG image
  vl2png     Convert a Vega-Lite specification to an PNG image
  vl2jpeg    Convert a Vega-Lite specification to an JPEG image
  vl2pdf     Convert a Vega-Lite specification to a PDF image
  vl2url     Convert a Vega-Lite specification to a URL that opens the chart in the Vega editor
  vl2html    Convert a Vega-Lite specification to an HTML file
  vg2svg     Convert a Vega specification to an SVG image
  vg2png     Convert a Vega specification to an PNG image
  vg2jpeg    Convert a Vega specification to an JPEG image
  vg2pdf     Convert a Vega specification to an PDF image
  vg2url     Convert a Vega specification to a URL that opens the chart in the Vega editor
  vg2html    Convert a Vega specification to an HTML file
  svg2png    Convert an SVG image to a PNG image
  svg2jpeg   Convert an SVG image to a JPEG image
  svg2pdf    Convert an SVG image to a PDF image
  ls-themes  List available themes
  cat-theme  Print the config JSON for a theme
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

For example, convert a Vega-Lite specification file named `in.vl.json` into an SVG file named `out.svg`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library.

```plain
$ vl-convert vl2svg -i ./in.vl.json -o ./out.svg --vl-version 5.8
```

## Python
Install the `vl-convert-python` package using pip

```
$ pip install vl-convert-python
```

Then in Python, import the library, use the `vegalite_to_png` function to convert a Vega-Lite specification string to a PNG image, then write the image to a file.

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
VlConvert relies on the standard Vega-Lite JavaScript library to perform the Vega-Lite to Vega conversion.  It uses the [Deno](https://deno.land/) project (in particular [`deno_runtime`](https://github.com/denoland/deno/tree/main/runtime)) to run Vega-Lite using the v8 JavaScript runtime, embedded into a Rust library called `vl-convert-rs`. This Rust library is then wrapped as a CLI application (`vl-convert`) and a Python library using PyO3 (`vl-convert-python`).

In addition to easy embedding of the v8 JavaScript runtime, another advantage of building on `deno_runtime` is that it's possible to customize the module loading logic. `vl-convert-rs` takes advantage of this to inline the minified JavaScript source code for multiple versions of Vega-Lite, and all their dependencies, into the Rust library itself. This way, no internet connection is required to use vl-convert, and the executable and Python library are truly self-contained.

## Vega-Lite to Vega conversion
The Vega-Lite to Vega compilation is performed directly by the Vega-Lite library running fully in the Deno runtime.

## Vega(-Lite) to SVG
The Vega JavaScript library supports exporting chart specifications to SVG images, and this conversion works in Deno. However, there is a subtle complication. In order to properly position text within the exported SVG, Vega needs to compute the width of text fragments (at a particular font size, in a particular font, etc.). When running in Node.js, these calculations are done using node canvas, which does not work in Deno. When node canvas is not available, Vega falls back to a rough heuristic for text measurement that results in poor text placement results.

VlConvert works around this by overriding the text width calculation function using a custom Rust function. This custom Rust function uses the `usvg` crate (part of the `resvg` project) to compute the width of text fragments.  With this customization, we regain accurate text placement in the SVG results produced by Vega.

## Vega(-Lite) to PNG
The Vega JavaScript library supports exporting chart specifications directly to PNG images. When running in Node.js, this functionality relies on node canvas, which is not available in Deno.

VlConvert generates PNG images by first exporting charts to SVG as described above, then converting the SVG image to a PNG image using the `resvg` crate.

## Vega(-Lite) to PDF
VlConvert generates PDF images by first exporting charts to SVG as described above, then converting them to PDF with a combination of the `svg2pdf` crate and custom text layout and font embedding logic. Font embedding currently supports TrueType fonts only.

## Limitations
### PNG Performance
VlConvert relies on the [`resvg`](https://github.com/RazrFalcon/resvg) Rust library for rendering PNG images from the SVG produced by Vega. resvg is a very accurate implementation of SVG rendering, but it is not GPU accelerated and can be somewhat slow when asked to render charts with many individual marks (e.g. large scatter plots).  For a single pane scatter plot, the performance is on the order of 1 second per 1000 points.

If PNG rendering performance is prohibitive, the recommended approach is to export to SVG and use another approach to convert the resulting SVG images to PNG.

### System font requirements
SVG text placement and PNG text rendering require that the fonts referenced by the exported chart are installed on the system that VlConvert is running on.  For example, when using the `vl-convert-python` package in [Google Colab](https://colab.research.google.com/), VlConvert will only have access to the limited set of fonts installed in the Colab kernel. It will not have access to the user fonts that the web browser has access to.

A directory containing additional font files can registered with the VlConvert Python library using the `vl_convert.register_font_directory` function. Similarly, the `--font-dir` argument can be used to register custom fonts in the `vl-convert` CLI application.

### Built-in Datasets
The [Vega Editor](https://vega.github.io/editor/) supports referring to built-in datasets as if the exist under a `data/` directory (e.g. `data/cars.json`). This is not currently supporte by VlConvert. Instead an absolute URL must be used (e.g. `https://raw.githubusercontent.com/vega/vega-datasets/next/data/cars.json`).


