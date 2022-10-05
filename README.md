# VlConvert
VlConvert provides a CLI utility and Python library for converting high-level Vega-Lite visualization specifications into low-level Vega visualization specifications, all with no web browser or system dependencies required.

# Getting started
## CLI
First, download the `vl-convert` release archive for your operating system from GitHub release page and unzip into the current directory.

```
$ ./vl-convert --help
```
```
vl-convert: A utility for converting Vega-Lite specifications into Vega specification

Usage: vl-convert [OPTIONS] --input-vegalite-file <INPUT_VEGALITE_FILE> --output-vega-file <OUTPUT_VEGA_FILE>

Options:
  -i, --input-vegalite-file <INPUT_VEGALITE_FILE>
          Path to input Vega-Lite file
  -o, --output-vega-file <OUTPUT_VEGA_FILE>
          Path to output Vega file to be created
  -v, --vl-version <VL_VERSION>
          Vega-Lite Version. One of 4.17, 5.0, 5.1, 5.2, 5.3, 5.4, 5.5 [default: 5.5]
  -p, --pretty
          Pretty-print JSON in output file
  -h, --help
          Print help information
  -V, --version
          Print version information
```

Example: Convert a Vega-Lite specification file named `in.vl.json` into a Vega specification file named `out.vg.json`. Perform the conversion using version 5.5 of the Vega-Lite JavaScript library and pretty-print the resulting JSON.
```
./vl-convert -i ./in.vl.json -o ./out.vg.json --vl-version 5.5 --pretty
```

## Python
Install the `vl-convert-python` pacakge using pip

```
$ pip install vl-convert-python
```

Then in Python, import the library, create a `VlConverter object`, and use the converter to convert a Vega-Lite specification string to a Vega specification string.

```python
from vl_convert import VlConverter

converter = VlConverter()

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

vg_spec = converter.vegalite_to_vega(vl_spec=vl_spec, vl_version="5.5", pretty=True)
print(vg_spec)
```
```
{
  "$schema": "https://vega.github.io/schema/vega/v5.json",
  "background": "white",
  "padding": 5,
  "width": 200,
  "height": 200,
  "style": "cell",
  ...
}
```

# Motivation
VlConvert was motivated by the needs of VegaFusion, which extracts data transformations from Vega specifications and evaluates them on the server. Using VlConvert, VegaFusion can input Vega-Lite specifications directly.  That said, VlConvert is designed to be used by the wider Vega-Lite ecosystem, independent of VegaFusion.

# How it works
VlConvert relies on the standard Vega-Lite JavaScript library to perform the Vega-Lite to Vega conversion.  It uses the Deno project (in particular `deno_core`) to run Vega-Lite using the v8 JavaScript runtime, embedded into a Rust library. This Rust library is then wrapped as a Python library using PyO3.

In addition to easy embedding of the v8 JavaScript runtime, another advantage of building on `deno_core` is that it's possible to customize the module loading logic. vl-convert takes advantage of this to inline the minified JavaScript source code for multiple versions of Vega-Lite, and all their dependencies, into the Rust library itself. This way, no internet connection is required to use vl-convert, and the executable and Python library are truly self-contained.

# Future work: Image Export
An exciting future possibility is to use the VlConvert infrastructure to enable zero-dependency image export of Vega(-Lite) visualizations.  This is already possible outside the web browser context using NodeJs, and Deno is designed to be largely compatible with NodeJs. The main difficulty is that Vega relies on the node canvas library for computing text metrics and performing png image export. Node Canvas is not compatible with Deno, so image export doesn't work out of the box.  