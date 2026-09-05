# vl-convert-python

`vl-convert-python` converts Vega-Lite, Vega, and SVG input from Python. For
Vega and Vega-Lite charts, it produces SVG, PNG, JPEG, PDF, HTML, scenegraphs,
font metadata, and Vega Editor URLs. It also compiles Vega-Lite to Vega and
converts existing SVG to PNG, JPEG, or PDF.

The package embeds the official Vega and Vega-Lite JavaScript libraries. It
does not require a browser, Node.js, or a separate rendering service.

## Installation

Install the package from PyPI:

```bash
pip install vl-convert-python
```

Python 3.7 and later are supported.

## Vega-Lite Example

Conversion functions accept a JSON-compatible dictionary or a JSON string:

```python
import vl_convert as vlc

spec = {
    "data": {
        "values": [
            {"category": "A", "value": 3},
            {"category": "B", "value": 7},
        ]
    },
    "mark": "bar",
    "encoding": {
        "x": {"field": "category", "type": "nominal"},
        "y": {"field": "value", "type": "quantitative"},
    },
}

svg = vlc.vegalite_to_svg(spec)
with open("chart.svg", "w", encoding="utf-8") as output_file:
    output_file.write(svg)

png = vlc.vegalite_to_png(spec, scale=2)
with open("chart.png", "wb") as output_file:
    output_file.write(png)
```

Use `vegalite_to_vega()` to inspect or save the compiled Vega specification:

```python
import json

vega_spec = vlc.vegalite_to_vega(spec)
with open("chart.vg.json", "w", encoding="utf-8") as output_file:
    json.dump(vega_spec, output_file)
```

## Altair Example

Pass the result of `Chart.to_dict()` directly to a Vega-Lite conversion:

```python
import altair as alt
import vl_convert as vlc

chart = (
    alt.Chart(alt.Data(values=[{"x": "A", "y": 3}, {"x": "B", "y": 7}]))
    .mark_bar()
    .encode(x="x:N", y="y:Q")
)

png = vlc.vegalite_to_png(chart.to_dict(), scale=2)
```

Select a bundled Vega-Lite compiler when a chart depends on an older release:

```python
svg = vlc.vegalite_to_svg(spec, vl_version="5.16")
```

## Configuration

`configure()` sets process-wide defaults. Conversion arguments override these
defaults for one call where the API provides a matching argument.

```python
import vl_convert as vlc

vlc.configure(
    num_workers=4,
    base_url=False,
    allowed_base_urls=["https://data.example.com/"],
    max_v8_heap_size_mb=512,
    max_v8_execution_time_secs=10,
)
```

Use `load_config()` to read the shared JSONC configuration format. Use
`get_config()` to inspect the active configuration.

## Fonts

VlConvert uses installed system fonts and accepts additional font directories.
Google Fonts downloads are opt-in. This example uses `spec` from the first
example:

```python
import vl_convert as vlc

vlc.configure(auto_google_fonts=True)
svg = vlc.vegalite_to_svg(spec)
```

Use `vegalite_fonts()` or `vega_fonts()` to inspect the fonts that VlConvert
resolves. These functions list local fonts only when `embed_local_fonts` is
enabled and list Google Fonts only when they are requested or automatic Google
Fonts are enabled.

## Asyncio

Awaitable conversion functions are available under `vl_convert.asyncio`. This
example uses `spec` from the first example:

```python
import asyncio
import vl_convert.asyncio as vlca


async def main():
    svg = await vlca.vegalite_to_svg(spec)
    print(svg[:5])


asyncio.run(main())
```

Configuration and cache-management helpers in the asyncio namespace are
synchronous re-exports. Do not await them.

## Network and File Access

The bundled JavaScript libraries need no network access. Specifications can
still request remote data and images, and optional Google Fonts or plugins can
make additional requests. Data and images share the `allowed_base_urls`
policy. Local files are blocked by default.

See the repository's
[documentation source](https://github.com/vega/vl-convert/tree/main/docs) for
the complete configuration and API guides.

## Development

From the repository root, build the extension in the Pixi environment and run
its tests:

```bash
pixi run dev-py
pixi run test-py
pixi run fmt-py-check
```
