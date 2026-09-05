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

::::{interface} python
```python
import vl_convert as vlc

with open("chart.svg", encoding="utf-8") as input_file:
    svg = input_file.read()

png = vlc.svg_to_png(svg)
pdf = vlc.svg_to_pdf(svg)
```

Both results are bytes. `svg_to_jpeg()` produces JPEG.
::::

::::{interface} cli
```bash
vl-convert svg2png --input chart.svg --output chart.png
vl-convert svg2pdf --input chart.svg --output chart.pdf
```

`svg2jpeg` produces JPEG. `svg2png` accepts `--scale` and `--ppi`, `svg2jpeg`
accepts `--scale` and `--quality`, and `svg2pdf` has no format options.
::::

::::{interface} rust
```rust
use vl_convert_rs::VlConverter;

let converter = VlConverter::new();
let output = converter.svg_to_png(svg, Default::default()).await?;
```

`svg` is a `&str` holding the document. The PNG bytes are in `output.data`.
::::

::::{interface} server
Send JSON with the markup in an `svg` string to `/svg/png`, `/svg/jpeg`, or
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

See {doc}`image-quality` for the `scale`, `ppi`, and `quality` options.
