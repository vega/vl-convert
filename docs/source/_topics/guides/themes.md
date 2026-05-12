---
title: Themes
path: guides/themes
section: Guides
order: 250
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Themes

Use built-in Vega themes or register custom theme config.

::::{interface} python
```python
import vl_convert as vlc

themes = vlc.get_themes()
svg = vlc.vegalite_to_svg(vl_spec, theme="dark")
```
::::

::::{interface} cli
```bash
vl-convert ls-themes
vl-convert cat-theme dark
vl-convert vl2svg --theme dark --input chart.vl.json --output chart.svg
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{VlConverter, VlOpts};

let opts = VlOpts {
    theme: Some("dark".to_string()),
    ..Default::default()
};
```
::::


::::{interface} server
```bash
curl http://localhost:3000/themes
```
::::
