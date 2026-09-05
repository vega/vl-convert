---
title: Image Size and Quality
path: guides/image-quality
section: Guides
order: 290
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Image Size and Quality

Choose vector output when possible. SVG works well for web pages and later
editing. PDF works well for documents and print. Both preserve shapes and text
without choosing a fixed pixel size.

Use PNG when the consumer requires pixels, lossless output, or transparency.
Use JPEG when smaller files matter more than lossless output and transparency.

## Chart Size and Pixel Density

`width` and `height` change the chart's logical dimensions before Vega lays out
axes, legends, titles, and padding. The final image can therefore be larger
than the requested plot dimensions.

`scale` multiplies the pixel dimensions of PNG and JPEG output without changing
the logical chart layout. A value of `2` is a common choice for high-density
displays.

PNG also accepts `ppi`, which sets pixels-per-inch metadata and contributes to
pixel dimensions:

```text
effective scale = scale * ppi / 72
```

The defaults are `scale=1` and `ppi=72`. Use `scale=2` or `ppi=144` to double
the pixel dimensions. Set both only when you want their effects multiplied.

JPEG accepts `quality` from `0` through `100` and defaults to `90`. Higher
values usually preserve more detail and produce larger files.

## Examples

::::{interface} python
```python
import vl_convert as vlc

png = vlc.vegalite_to_png(spec, width=640, height=360, scale=2)
jpeg = vlc.vegalite_to_jpeg(spec, width=640, height=360, quality=90)
```

Both functions return image bytes. The example assumes `spec` is a Vega-Lite
dictionary such as the one in {doc}`../getting-started/quick-start`.
::::

::::{interface} cli
```bash
vl-convert vl2png \
  --input chart.vl.json --output chart.png \
  --width 640 --height 360 --scale 2

vl-convert vl2jpeg \
  --input chart.vl.json --output chart.jpg \
  --width 640 --height 360 --quality 90
```

The example uses `chart.vl.json` from
{doc}`../getting-started/quick-start`.
::::

::::{interface} rust
```rust
use vl_convert_rs::{JpegOpts, PngOpts, VlOpts};

let chart_size = VlOpts {
    width: Some(640.0),
    height: Some(360.0),
    ..Default::default()
};

let png = converter
    .vegalite_to_png(
        spec.clone(),
        chart_size.clone(),
        PngOpts {
            scale: Some(2.0),
            ..Default::default()
        },
    )
    .await?;

let jpeg = converter
    .vegalite_to_jpeg(
        spec,
        chart_size,
        JpegOpts {
            quality: Some(90),
            ..Default::default()
        },
    )
    .await?;
```

The example assumes `converter` and `spec` are defined as in
{doc}`../getting-started/quick-start`.
::::

::::{interface} server
Put sizing options beside `spec` in the request body. For example, save this
body as `request.json`:

```json
{
  "spec": {
    "data": {"values": [{"x": "A", "y": 2}, {"x": "B", "y": 5}]},
    "mark": "bar",
    "encoding": {
      "x": {"field": "x", "type": "nominal"},
      "y": {"field": "y", "type": "quantitative"}
    }
  },
  "width": 640,
  "height": 360,
  "scale": 2
}
```

```bash
curl http://127.0.0.1:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @request.json \
  --output chart.png
```
::::

Vega and Vega-Lite PNG output uses Vega's canvas renderer. JPEG and PDF outputs
are produced from SVG. Direct SVG input conversions use the Rust image and PDF
renderers without running Vega.
