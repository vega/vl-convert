---
title: Themes
path: guides/themes
section: Guides
order: 250
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Themes

A theme is a named Vega configuration object applied while Vega-Lite compiles a
specification. Themes can set defaults for colors, marks, axes, legends, fonts,
and other visual properties. They apply to Vega-Lite input, not to an already
compiled Vega specification.

VlConvert includes the themes from the `vega-themes` package. Select a theme
for one conversion, or set `default_theme` in converter configuration.

## Use a Built-In Theme

::::{interface} python
```python
import vl_convert as vlc

print(vlc.get_themes())
svg = vlc.vegalite_to_svg(spec, theme="dark")
```

`get_themes()` returns the available names. `get_theme("dark")` returns the
corresponding Vega configuration.
::::

::::{interface} cli
```bash
vl-convert ls-themes
vl-convert cat-theme dark
vl-convert vl2svg \
  --input chart.vl.json --output chart.svg \
  --theme dark
```
::::

::::{interface} rust
Set `VlOpts.theme` for the conversion:

```rust
let options = VlOpts {
    theme: Some("dark".to_string()),
    ..Default::default()
};

let output = converter
    .vegalite_to_svg(spec, options, Default::default())
    .await?;
```
::::

::::{interface} server
`GET /themes` lists available names, and `GET /themes/{name}` returns one
theme:

```bash
curl http://127.0.0.1:3000/themes
curl http://127.0.0.1:3000/themes/dark
```

Set `theme` beside `spec` in a Vega-Lite conversion request.
::::

## Register a Custom Theme

A custom theme file is a JSON object whose keys are theme names and whose values
are Vega configuration objects. Save this example as `themes.json`:

```json
{
  "brand": {
    "background": "white",
    "range": {"category": ["#225ea8", "#41b6c4", "#a1dab4"]},
    "axis": {
      "labelFont": "Inter",
      "titleFont": "Inter"
    }
  }
}
```

A custom theme takes priority when its name matches a built-in theme.

::::{interface} python
```python
import json
import vl_convert as vlc

with open("themes.json", encoding="utf-8") as input_file:
    themes = json.load(input_file)

vlc.configure(themes=themes)
svg = vlc.vegalite_to_svg(spec, theme="brand")
```
::::

::::{interface} cli
```bash
vl-convert --themes themes.json \
  vl2svg --theme brand --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
```rust
use std::collections::HashMap;
use vl_convert_rs::{serde_json, VlcConfig, VlConverter};

let themes: HashMap<String, serde_json::Value> =
    serde_json::from_str(include_str!("../themes.json"))?;

let converter = VlConverter::with_config(VlcConfig {
    themes,
    ..Default::default()
})?;
```
::::

::::{interface} server
Register custom themes before the `serve` subcommand:

```bash
vl-convert --themes themes.json \
  serve --host 127.0.0.1 --port 3000
```

The themes are then available through `GET /themes` and Vega-Lite conversion
requests.
::::

See {doc}`../advanced/configuration` to set a persistent default theme.
