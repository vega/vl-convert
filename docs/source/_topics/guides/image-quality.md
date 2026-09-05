---
title: Image Size and Quality
path: guides/image-quality
section: Guides
order: 230
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Image Size and Quality

Prefer vector output when the consumer supports it. SVG suits web pages and
later editing, and PDF suits documents and print. Neither needs a fixed pixel
size.

Use PNG when the consumer needs pixels, lossless output, or transparency. Use
JPEG when a smaller file matters more than either.

## Chart Size

`width` and `height` set the chart's logical dimensions before Vega adds axes,
legends, titles, and padding, so the final image is usually larger than the
values you pass. They override the top-level `width` and `height` of the
specification.

## Pixel Density

`scale` multiplies the pixel dimensions of PNG and JPEG output without changing
the layout. `2` is a common choice for high-density displays.

PNG also accepts `ppi`, which records pixels-per-inch metadata in the file and
contributes to the pixel dimensions:

```text
effective scale = scale * ppi / 72
```

The defaults are `scale=1` and `ppi=72`. Either `scale=2` or `ppi=144` doubles
the pixel dimensions. Set both only when you want the effects multiplied.

## JPEG Quality

`quality` ranges from `0` through `100` and defaults to `90`. Higher values keep
more detail and produce larger files.

## Examples

::::{interface} python
```python
import vl_convert as vlc

png = vlc.vegalite_to_png(spec, width=640, height=360, scale=2)
jpeg = vlc.vegalite_to_jpeg(spec, width=640, height=360, quality=90)
```

Both functions return bytes. `spec` is a Vega-Lite dictionary such as the one
in {doc}`../getting-started/quick-start`.
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

The example uses `chart.vl.json` from {doc}`../getting-started/quick-start`.
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

`converter` and `spec` are defined as in {doc}`../getting-started/quick-start`.
::::

::::{interface} server
Put the options beside `spec` in the request body. For example, save this body
as `request.json`:

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

PNG output uses Vega's canvas renderer. JPEG and PDF are produced from the SVG
rendering. SVG input is converted directly by Rust image and PDF libraries
without running Vega, and `width` and `height` do not apply to it. See
{doc}`../advanced/conversion-overrides` for every per-conversion option.
