---
title: Font Introspection
path: advanced/font-introspection
section: Advanced
order: 460
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Font Introspection

Font introspection returns the fonts and variants a spec needs after Vega
layout resolves styles.

::::{interface} python
```python
fonts = vlc.vegalite_fonts(vl_spec)
```
::::

::::{interface} cli
```bash
vl-convert vl2fonts --input chart.vl.json --output fonts.json
vl-convert vg2fonts --input chart.vg.json --output fonts.json
```
::::


::::{interface} rust
```rust
let fonts = converter.vegalite_fonts(spec, Default::default()).await?;
```
::::


::::{interface} server
```bash
curl -X POST http://localhost:3000/vegalite/fonts \
  -H 'Content-Type: application/json' \
  --data-binary @chart.vl.json
```
::::
