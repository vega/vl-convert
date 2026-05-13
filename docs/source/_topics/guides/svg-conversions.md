---
title: SVG Conversions
path: guides/svg-conversions
section: Guides
order: 220
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting SVG

SVG conversions skip Vega and V8. Use them for post-processing existing SVG or
rendering SVG produced by another tool.

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
jq -Rs '{svg: .}' chart.svg |
  curl -X POST http://localhost:3000/svg/png \
  -H 'Content-Type: application/json' \
  --data-binary @- > chart.png
```
::::
