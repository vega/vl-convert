---
title: Themes
path: guides/themes
section: Guides
order: 250
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Themes

Themes are Vega config objects applied during Vega-Lite compilation. Built-in
themes come from `vega-themes`; custom themes are registered by name and can
override built-ins with the same name.

For persistent theme defaults and config-file loading, see
{doc}`../advanced/configuration`.

::::{interface} python
```python
import vl_convert as vlc

themes = vlc.get_themes()
svg = vlc.vegalite_to_svg(vl_spec, theme="dark")

vlc.configure(themes={
    "brand": {
        "background": "white",
        "axis": {"labelFont": "Inter", "titleFont": "Inter"},
    }
})
svg = vlc.vegalite_to_svg(vl_spec, theme="brand")
```
::::

::::{interface} cli
```bash
vl-convert ls-themes
vl-convert cat-theme dark
vl-convert vl2svg --theme dark --input chart.vl.json --output chart.svg

vl-convert --themes themes.jsonc \
  vl2svg --theme brand --input chart.vl.json --output chart.svg
```
::::


::::{interface} rust
```rust
use std::collections::HashMap;
use vl_convert_rs::{VlConverter, VlcConfig, VlOpts};

let converter = VlConverter::with_config(VlcConfig {
    themes: HashMap::from([(
        "brand".to_string(),
        serde_json::json!({
            "background": "white",
            "axis": {"labelFont": "Inter", "titleFont": "Inter"}
        }),
    )]),
    ..Default::default()
})?;

let opts = VlOpts {
    theme: Some("brand".to_string()),
    ..Default::default()
};

let output = converter.vegalite_to_svg(spec, opts, Default::default()).await?;
```
::::


::::{interface} server
```bash
curl http://localhost:3000/themes

curl http://localhost:3000/themes/dark

vl-convert --themes themes.jsonc serve \
  --host 127.0.0.1 \
  --port 3000
```
::::

`themes.jsonc` is a JSON object whose keys are theme names and whose values are
Vega config objects:

```json
{
  "brand": {
    "background": "white",
    "axis": {
      "labelFont": "Inter",
      "titleFont": "Inter"
    }
  }
}
```
