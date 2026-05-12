---
title: Quick Start
path: getting-started/quick-start
section: Getting Started
order: 110
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Quick Start

Convert a Vega-Lite specification to PNG.

::::{interface} python
Conversion functions return bytes. Write the result to a file, send it in a
response, or pass it to another Python library.

```python
import vl_convert as vlc

spec = {
    "data": {"values": [{"a": "A", "b": 2}, {"a": "B", "b": 5}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "a", "type": "nominal"},
        "y": {"field": "b", "type": "quantitative"},
    },
}

png = vlc.vegalite_to_png(spec, scale=2)
with open("chart.png", "wb") as f:
    f.write(png)
```
::::

::::{interface} cli
CLI conversion commands write rendered output to `--output`. Use `--input -`
and `--output -` when composing commands with stdin or stdout.

```bash
vl-convert vl2png --input chart.vl.json --output chart.png --scale 2
```

```bash
vl-convert vl2png --input - --output - < chart.vl.json > chart.png
```
::::


::::{interface} rust
Rust conversion methods return output structs. The rendered bytes are available
on `output.data`.

```rust
use vl_convert_rs::{VlConverter, VlOpts};

let converter = VlConverter::new();
let output = converter.vegalite_to_png(spec, VlOpts::default(), Default::default()).await?;
std::fs::write("chart.png", output.data)?;
```
::::


::::{interface} server
The HTTP endpoint returns the rendered PNG bytes in the response body.

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

```bash
curl -X POST http://127.0.0.1:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @chart.vl.json > chart.png
```
::::
