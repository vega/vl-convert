---
title: Converting SVG
path: guides/svg-conversions
section: Guides
order: 220
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting SVG

Use the SVG functions when another tool has already produced the vector image.
VlConvert converts it to PNG, JPEG, or PDF without running Vega.

The SVG must declare its size through `width` and `height` attributes or a
`viewBox`. Fonts and images it references must be available to the rendering
process. See {doc}`fonts` and {doc}`security` when the SVG is not
self-contained.

Save this SVG document:

:::{dropdown} chart.svg
:open:

```{literalinclude} /_examples/svg-demo.svg
:language: xml
```
:::

::::{interface} python
```python
from pathlib import Path

import vl_convert as vlc

svg = Path("chart.svg").read_text(encoding="utf-8")

png = vlc.svg_to_png(svg, scale=2)
pdf = vlc.svg_to_pdf(svg)
Path("chart.png").write_bytes(png)
Path("chart.pdf").write_bytes(pdf)
```

Both results are bytes. `svg_to_jpeg()` produces JPEG.
::::

::::{interface} cli
```bash
vl-convert svg2png --input chart.svg --output chart.png --scale 2
vl-convert svg2pdf --input chart.svg --output chart.pdf
```

`svg2jpeg` produces JPEG. `svg2png` accepts `--scale` and `--ppi`, `svg2jpeg`
accepts `--scale` and `--quality`, and `svg2pdf` has no format options.
::::

::::{interface} rust
```rust
use vl_convert_rs::{PngOpts, VlConverter};

let svg = std::fs::read_to_string("chart.svg")?;
let converter = VlConverter::new();
let output = converter
    .svg_to_png(
        &svg,
        PngOpts {
            scale: Some(2.0),
            ..Default::default()
        },
    )
    .await?;
std::fs::write("chart.png", output.data)?;
```

The PNG bytes are in `output.data`.
::::

::::{interface} server
Send JSON with the markup in an `svg` string to `/svg/png`, `/svg/jpeg`, or
`/svg/pdf`. Save this complete request as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/svg-png.json
:language: json
```
:::

```bash
curl http://127.0.0.1:3000/svg/png \
  -H 'Content-Type: application/json' \
  --data-binary @request.json \
  --output chart.png
```
::::

The PNG conversion preserves the shapes and colors in the SVG document:

```{vl-chart} /_examples/svg-demo.svg
:input-kind: svg
:format: png
:scale: 2
:alt: A blue circle, orange triangle, and green square on a pale background
```

See {doc}`image-quality` for the `scale`, `ppi`, and `quality` options.
