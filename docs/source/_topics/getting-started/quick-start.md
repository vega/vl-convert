---
title: Quick Start
path: getting-started/quick-start
section: Getting Started
order: 110
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Quick Start

This example renders a small Vega-Lite bar chart as `chart.png`.

::::{interface} python
`vegalite_to_png()` returns PNG bytes. Write the result to a file, send it in a
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
with open("chart.png", "wb") as output_file:
    output_file.write(png)
```

Run the script. The `scale=2` argument doubles the output's pixel dimensions.
See {doc}`/python/guides/vegalite-conversions` for other output formats.
::::

::::{interface} cli
Save this specification as `chart.vl.json`:

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {
    "values": [
      {"category": "A", "value": 2},
      {"category": "B", "value": 5}
    ]
  },
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "value", "type": "quantitative"}
  }
}
```

Render the file at twice its default pixel dimensions:

```bash
vl-convert vl2png --input chart.vl.json --output chart.png --scale 2
```

The command creates `chart.png`. CLI conversion commands also accept standard
input and output. For example:

```bash
vl-convert vl2png --input - --output - < chart.vl.json > chart.png
```

See {doc}`/cli/guides/vegalite-conversions` for other output formats and
{doc}`/cli/advanced/cli-piping` for pipeline examples.
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
                {"category": "A", "value": 2},
                {"category": "B", "value": 5}
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

Run `cargo run`. Rust conversion methods return output structs, and `data`
contains the rendered bytes. See {doc}`/rust/guides/vegalite-conversions` for
other output formats.
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
        {"category": "A", "value": 2},
        {"category": "B", "value": 5}
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

Send the request from a second terminal:

```bash
curl http://127.0.0.1:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @request.json \
  --output chart.png
```

The endpoint returns PNG bytes and `curl` writes them to `chart.png`. See
{doc}`/server/guides/vegalite-conversions` for the other Vega-Lite endpoints.
::::
