# vl-convert-python
This crate uses [PyO3](https://pyo3.rs/) to wrap the `vl-convert-rs` crate as a Python library.

# Installation
`vl-convert-python` can be installed using pip with

```
$ pip install vl-convert-python
```

# Usage
From Python, import the `vl_convert` package, create a `VlConverter` object, and use the converter to convert a Vega-Lite specification string to a Vega specification string.

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


## Python development setup
Create development conda environment
```
$ conda create -n vl-convert-dev -c conda-forge python=3.10 deno maturin pytest black
```

Activate environment
```
$ conda activate vl-convert-dev
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
