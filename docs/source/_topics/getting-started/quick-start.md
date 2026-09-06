---
title: Quick Start
path: getting-started/quick-start
section: Getting Started
order: 110
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Quick Start

This example renders a small Vega-Lite bar chart as `chart.png`. The data is
inline. See {doc}`../guides/data-loading` when a chart loads data from a URL
or file.

::::{interface} python
`vegalite_to_png()` takes a Vega-Lite specification as a dictionary or JSON
string and returns PNG bytes:

```python
import vl_convert as vlc

spec = {
    "data": {
        "values": [
            {"category": "A", "value": 28},
            {"category": "B", "value": 55},
            {"category": "C", "value": 43},
            {"category": "D", "value": 91},
            {"category": "E", "value": 81},
        ]
    },
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

`scale=2` doubles the pixel dimensions of the output. See
{doc}`../guides/vegalite-conversions` for the other output formats.
::::

::::{interface} cli
Save this specification as `chart.vl.json`:

```{literalinclude} /_examples/quick-start.vl.json
:language: json
```

Render it at twice the default pixel dimensions:

```bash
vl-convert vl2png --input chart.vl.json --output chart.png --scale 2
```

Conversion commands also read standard input and write standard output:

```bash
vl-convert vl2png --input - --output - < chart.vl.json > chart.png
```

See {doc}`../guides/vegalite-conversions` for the other output formats and
{doc}`../advanced/cli-piping` for pipeline usage.
::::

::::{interface} rust
Add these dependencies to `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
vl-convert-rs = "2"
```

Use this complete `src/main.rs`:

```rust
use vl_convert_rs::{anyhow, serde_json, PngOpts, VlConverter, VlOpts};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {
            "values": [
                {"category": "A", "value": 28},
                {"category": "B", "value": 55},
                {"category": "C", "value": 43},
                {"category": "D", "value": 91},
                {"category": "E", "value": 81}
            ]
        },
        "mark": "bar",
        "encoding": {
            "x": {"field": "category", "type": "nominal"},
            "y": {"field": "value", "type": "quantitative"}
        }
    });

    let converter = VlConverter::new();
    let output = converter
        .vegalite_to_png(
            spec,
            VlOpts::default(),
            PngOpts {
                scale: Some(2.0),
                ..Default::default()
            },
        )
        .await?;
    std::fs::write("chart.png", output.data)?;
    Ok(())
}
```

Run `cargo run`. Conversion methods return output structs, and `data` holds
the rendered bytes. See {doc}`../guides/vegalite-conversions` for the other
output formats and {doc}`../advanced/rust-converter` for reusing the converter.
::::

::::{interface} server
Start the server in one terminal:

```bash
vl-convert serve --host 127.0.0.1 --port 3000
```

Save this request body as `request.json`:

```json
{
  "spec": {
    "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
    "data": {
      "values": [
        {"category": "A", "value": 28},
        {"category": "B", "value": 55},
        {"category": "C", "value": 43},
        {"category": "D", "value": 91},
        {"category": "E", "value": 81}
      ]
    },
    "mark": "bar",
    "encoding": {
      "x": {"field": "category", "type": "nominal"},
      "y": {"field": "value", "type": "quantitative"}
    }
  },
  "scale": 2
}
```

Send it from a second terminal:

```bash
curl http://127.0.0.1:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @request.json \
  --output chart.png
```

The endpoint returns PNG bytes, which `curl` writes to `chart.png`. See
{doc}`../guides/vegalite-conversions` for the other endpoints and
{doc}`../overview` for what the server provides.
::::

The specification renders as this chart:

```{vl-chart} /_examples/quick-start.vl.json
:alt: Bar chart with five bars labeled A through E
```
