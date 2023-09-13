# Overview
`vl-convert-python` is a dependency-free Python package for converting [Vega-Lite](https://vega.github.io/vega-lite/) chart specifications into static images (SVG or PNG) or [Vega](https://vega.github.io/vega/) chart specifications.


Since an Altair chart can generate Vega-Lite, this package can be used to easily create static images from Altair charts.

Try it out on Binder! \
[![Binder](https://mybinder.org/badge_logo.svg)](https://mybinder.org/v2/gh/jonmmease/vl-convert/main?labpath=vl-convert-python%2Fnotebooks%2Fconvert_vegalite.ipynb)

# Installation
`vl-convert-python` can be installed using pip with

```
$ pip install vl-convert-python
```

# Usage
The `vl-convert-python` package provides a series of conversion functions under the `vl_convert` module.

## Convert Vega-Lite to SVG, PNG, and Vega
The `vegalite_to_svg` and `vegalite_to_png` functions can be used to convert Vega-Lite specifications to static SVG and PNG images respectively. The `vegalite_to_vega` function can be used to convert a Vega-Lite specification to a Vega specification.

```python
import vl_convert as vlc
import json

vl_spec = r"""
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/next/data/movies.json"},
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

# Create SVG image string and then write to a file
svg_str = vlc.vegalite_to_svg(vl_spec=vl_spec)
with open("chart.svg", "wt") as f:
    f.write(svg_str)

# Create PNG image data and then write to a file
png_data = vlc.vegalite_to_png(vl_spec=vl_spec, scale=2)
with open("chart.png", "wb") as f:
    f.write(png_data)

# Create low-level Vega representation of chart and write to file
vg_spec = vlc.vegalite_to_vega(vl_spec)
with open("chart.vg.json", "wt") as f:
    json.dump(vg_spec, f)
```

## Convert Altair Chart to SVG, PNG, and Vega
The Altair visualization library provides a Pythonic API for generating Vega-Lite visualizations. As such, `vl-convert-python` can be used to convert Altair charts to PNG, SVG, or Vega. The `vegalite_*` functions support an optional `vl_version` argument that can be used to specify the particular version of the Vega-Lite JavaScript library to use.  Version 4.2 of the Altair package uses Vega-Lite version 4.17, so this is the version that should be specified when converting Altair charts.

```python
import altair as alt
from vega_datasets import data
import vl_convert as vlc
import json

source = data.barley()

chart = alt.Chart(source).mark_bar().encode(
    x='sum(yield)',
    y='variety',
    color='site'
)

# Create SVG image string and then write to a file
svg_str = vlc.vegalite_to_svg(chart.to_json(), vl_version="4.17")
with open("altair_chart.svg", "wt") as f:
    f.write(svg_str)

# Create PNG image data and then write to a file
png_data = vlc.vegalite_to_png(chart.to_json(), vl_version="4.17", scale=2)
with open("altair_chart.png", "wb") as f:
    f.write(png_data)

# Create low-level Vega representation of chart and write to file
vg_spec = vlc.vegalite_to_vega(chart.to_json(), vl_version="4.17")
with open("altair_chart.vg.json", "wt") as f:
    json.dump(vg_spec, f)
```
# How it works
This crate uses [PyO3](https://pyo3.rs/) to wrap the [`vl-convert-rs`](https://crates.io/crates/vl-convert-rs) Rust crate as a Python library. The `vl-convert-rs` crate is a self-contained Rust library for converting [Vega-Lite](https://vega.github.io/vega-lite/) visualization specifications into various formats.  The conversions are performed using the Vega-Lite and Vega JavaScript libraries running in a v8 JavaScript runtime provided by the [`deno_runtime`](https://crates.io/crates/deno_runtime) crate.  Font metrics and SVG-to-PNG conversions are provided by the [`resvg`](https://crates.io/crates/resvg) crate.

Of note, `vl-convert-python` is fully self-contained and has no dependency on an external web browser or Node.js runtime.

# Development setup
Create development conda environment
```
$ conda create -n vl-convert-dev -c conda-forge python=3.10 deno maturin altair pytest black black-jupyter scikit-image
```

Activate environment and pip install remaining dependencies
```
$ conda activate vl-convert-dev
$ pip install pypdfium2
```

Change to Python package directory
```
$ cd vl-convert-python

```
Build Rust python package with maturin in develop mode
```
$ maturin develop --release
```

Run tests
```
$ pytest tests
```
