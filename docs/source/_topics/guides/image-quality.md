---
title: Image Quality
path: guides/image-quality
section: Guides
order: 290
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Image Quality

Raster output has two separate concerns: chart layout and pixel density.
`width`, `height`, and `background` change the Vega or Vega-Lite spec before
rendering. `scale` and `ppi` control the rasterization step after the spec has
produced SVG.

For PNG output, vl-convert uses `effective_scale = scale * ppi / 72`. The
defaults are `scale=1` and `ppi=72`, so `scale=2` doubles pixel dimensions and
`ppi=144` also doubles them. Use one of those knobs unless you intentionally
need both larger pixels and non-default PPI metadata. JPEG uses `scale` and
`quality`; `quality` is `0..100` and defaults to `90`. PDF output is vector
output and does not use scale.

::::{interface} python
```python
png = vlc.vegalite_to_png(vl_spec, scale=2, width=640, height=360)
jpeg = vlc.vegalite_to_jpeg(vl_spec, scale=2, quality=90)
```
::::

::::{interface} cli
```bash
vl-convert vl2png --scale 2 --width 640 --height 360 \
  --input chart.vl.json --output chart.png
vl-convert vl2jpeg --scale 2 --quality 90 \
  --input chart.vl.json --output chart.jpg
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{JpegOpts, PngOpts, VlOpts};

let vl_opts = VlOpts {
    width: Some(640.0),
    height: Some(360.0),
    ..Default::default()
};

let png_opts = PngOpts {
    scale: Some(2.0),
    ppi: None,
};
let jpeg_opts = JpegOpts {
    scale: Some(2.0),
    quality: Some(90),
};
```
::::


::::{interface} server
```bash
jq -c '{spec: ., scale: 2, width: 640, height: 360}' chart.vl.json |
  curl -X POST http://localhost:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @- > chart.png
```
::::

Use SVG output when the consumer can render vector graphics directly. It avoids
raster density tradeoffs and can still be converted to PNG, JPEG, or PDF later.
