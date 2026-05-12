---
title: Converting Vega-Lite
path: guides/vegalite-conversions
section: Guides
order: 200
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting Vega-Lite

Use Vega-Lite inputs when the source specification uses the Vega-Lite schema.

::::{interface} python
```python
import vl_convert as vlc

svg = vlc.vegalite_to_svg(vl_spec)
png = vlc.vegalite_to_png(vl_spec, scale=2)
vg = vlc.vegalite_to_vega(vl_spec)
```
::::

::::{interface} cli
```bash
vl-convert vl2svg --input chart.vl.json --output chart.svg
vl-convert vl2png --input chart.vl.json --output chart.png --scale 2
vl-convert vl2vg --input chart.vl.json --output chart.vg.json --pretty
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{VlConverter, VlOpts};

let converter = VlConverter::new();
let svg = converter.vegalite_to_svg(spec.clone(), VlOpts::default(), Default::default()).await?;
let png = converter.vegalite_to_png(spec, VlOpts::default(), Default::default()).await?;
```
::::


::::{interface} server
```bash
curl -X POST http://localhost:3000/vegalite/svg \
  -H 'Content-Type: application/json' \
  --data-binary @chart.vl.json > chart.svg
```
::::
