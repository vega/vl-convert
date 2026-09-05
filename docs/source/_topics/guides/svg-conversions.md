---
title: SVG Conversions
path: guides/svg-conversions
section: Guides
order: 220
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting SVG

Use an SVG input when another tool has already produced the vector image.
VlConvert can turn it into PNG, JPEG, or PDF without parsing a Vega or
Vega-Lite specification.

The SVG must include enough sizing information to establish an image size.
Referenced fonts and images must be available to the rendering process. See
{doc}`fonts` and {doc}`security` when the SVG is not fully self-contained.

::::{interface} python
```python
import vl_convert as vlc

with open("chart.svg", encoding="utf-8") as input_file:
    svg = input_file.read()

png = vlc.svg_to_png(svg)
pdf = vlc.svg_to_pdf(svg)
```

Both results are bytes. Use `svg_to_jpeg()` when JPEG output is required.
::::

::::{interface} cli
```bash
vl-convert svg2png --input chart.svg --output chart.png
vl-convert svg2pdf --input chart.svg --output chart.pdf
```

`svg2jpeg` provides JPEG output. PNG and JPEG accept raster scaling options.
::::

::::{interface} rust
```rust
use vl_convert_rs::VlConverter;

let converter = VlConverter::new();
let output = converter.svg_to_png(svg, Default::default()).await?;
```

`svg` is a string containing the input document. The PNG bytes are in
`output.data`.
::::

::::{interface} server
Send JSON with an `svg` string to `/svg/png`, `/svg/jpeg`, or
`/svg/pdf`. For example, save this request as `request.json`:

```json
{
  "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"120\" height=\"40\"><rect width=\"120\" height=\"40\" fill=\"steelblue\"/></svg>",
  "scale": 2
}
```

```bash
curl http://127.0.0.1:3000/svg/png \
  -H 'Content-Type: application/json' \
  --data-binary @request.json \
  --output chart.png
```
::::
