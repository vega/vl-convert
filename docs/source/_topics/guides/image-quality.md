---
title: Image Quality
path: guides/image-quality
section: Guides
order: 290
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Image Quality

Use scale, PPI, and JPEG quality controls for raster output. PDF output is
vector output and does not use scale.

::::{interface} python
```python
png = vlc.vegalite_to_png(vl_spec, scale=2, ppi=144)
jpeg = vlc.vegalite_to_jpeg(vl_spec, quality=90)
```
::::

::::{interface} cli
```bash
vl-convert vl2png --scale 2 --ppi 144 --input chart.vl.json --output chart.png
vl-convert vl2jpeg --quality 90 --input chart.vl.json --output chart.jpg
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{PngOpts, JpegOpts};

let png_opts = PngOpts {
    scale: Some(2.0),
    ppi: Some(144.0),
};
let jpeg_opts = JpegOpts {
    scale: Some(2.0),
    quality: Some(90),
};
```
::::


::::{interface} server
```bash
jq -c '{spec: ., scale: 2, ppi: 144}' chart.vl.json |
  curl -X POST http://localhost:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @- > chart.png
```
::::
