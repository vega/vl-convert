---
title: Converting Vega
path: guides/vega-conversions
section: Guides
order: 210
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting Vega

Use Vega inputs when the specification is already compiled to Vega.

::::{interface} python
```python
import vl_convert as vlc

svg = vlc.vega_to_svg(vg_spec)
png = vlc.vega_to_png(vg_spec)
```
::::

::::{interface} cli
```bash
vl-convert vg2svg --input chart.vg.json --output chart.svg
vl-convert vg2png --input chart.vg.json --output chart.png
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{VgOpts, VlConverter};

let converter = VlConverter::new();
let output = converter.vega_to_svg(spec, VgOpts::default(), Default::default()).await?;
```
::::


::::{interface} server
```bash
curl -X POST http://localhost:3000/vega/png \
  -H 'Content-Type: application/json' \
  --data-binary @chart.vg.json > chart.png
```
::::
