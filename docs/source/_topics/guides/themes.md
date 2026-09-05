---
title: Themes
path: guides/themes
section: Guides
order: 250
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Themes

A theme is a named Vega configuration object that Vega-Lite applies while
compiling a specification. Themes set defaults for colors, marks, axes,
legends, fonts, and other visual properties. They apply to Vega-Lite input
only. A compiled Vega specification already contains its configuration.

VlConvert bundles the themes from the `vega-themes` package. Select a theme per
conversion, or set `default_theme` in the converter configuration.

## Use a Built-In Theme

::::{interface} python
```python
import vl_convert as vlc

print(list(vlc.get_themes()))
svg = vlc.vegalite_to_svg(spec, theme="dark")
```

`get_themes()` maps each theme name to its Vega configuration, so
`vlc.get_themes()["dark"]` is the configuration that the `dark` theme applies.
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
`GET /themes` lists the available names, and `GET /themes/{name}` returns one
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

A custom theme replaces a built-in theme with the same name.

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

The themes then appear in `GET /themes` and can be selected by Vega-Lite
conversion requests.
::::

See {doc}`../advanced/configuration` to set a default theme for every
conversion, and {doc}`../advanced/conversion-overrides` to pass a raw `config`
object instead of a named theme.
