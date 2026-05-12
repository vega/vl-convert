---
title: SVG Conversions
path: guides/svg-conversions
section: Guides
order: 220
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting SVG

Use SVG inputs when Vega has already produced SVG or another tool owns SVG
generation.

::::{interface} python
```python
import vl_convert as vlc

png = vlc.svg_to_png(svg)
pdf = vlc.svg_to_pdf(svg)
```
::::

::::{interface} cli
```bash
vl-convert svg2png --input chart.svg --output chart.png
vl-convert svg2pdf --input chart.svg --output chart.pdf
```
::::


::::{interface} rust
```rust
use vl_convert_rs::VlConverter;

let converter = VlConverter::new();
let output = converter.svg_to_png(svg, Default::default()).await?;
```
::::


::::{interface} server
```bash
curl -X POST http://localhost:3000/svg/png \
  -H 'Content-Type: image/svg+xml' \
  --data-binary @chart.svg > chart.png
```
::::
