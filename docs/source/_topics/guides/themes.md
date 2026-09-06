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
only because Vega input skips Vega-Lite compilation. For Vega input, pass a
configuration object directly with `config` or `--config`.

VlConvert bundles the themes from the `vega-themes` package. Select a theme per
conversion, or set `default_theme` in the converter configuration.

## Use a Built-In Theme

::::{interface} python
```python
import vl_convert as vlc

svg = vlc.vegalite_to_svg(spec, theme="dark")
```
::::

::::{interface} cli
```bash
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
Set `theme` beside `spec` in a Vega-Lite conversion request:

```json
{
  "spec": {"mark": "bar", "data": {"values": []}},
  "theme": "dark"
}
```
::::

The same chart with the default configuration and with the `dark` theme:

::::{grid} 1 1 2 2
:gutter: 3

:::{grid-item}
```{vl-chart} /_examples/theme-demo.vl.json
:alt: Stacked bar chart of revenue by quarter with the default configuration
```
:::

:::{grid-item}
```{vl-chart} /_examples/theme-demo.vl.json
:theme: dark
:alt: The same stacked bar chart with the dark theme
```
:::
::::

## List and Inspect Themes

::::{interface} python
`get_themes()` maps each theme name to the Vega configuration it applies:

```python
themes = vlc.get_themes()
print(sorted(themes))
print(themes["dark"])
```
::::

::::{interface} cli
`ls-themes` prints the available names:

```bash
vl-convert ls-themes
```

```{program-output} python ../tools/run_vl_convert.py ls-themes
```

`cat-theme` prints the configuration a theme applies. `dark` is the shortest:

```bash
vl-convert cat-theme dark
```

```{program-output} python ../tools/run_vl_convert.py cat-theme dark
```
::::

::::{interface} rust
`get_themes()` returns the same mapping as a JSON value:

```rust
let themes = converter.get_themes().await?;
println!("{}", themes["dark"]);
```
::::

::::{interface} server
`GET /themes` lists the available names, and `GET /themes/{name}` returns the
configuration one theme applies:

```bash
curl http://127.0.0.1:3000/themes
curl http://127.0.0.1:3000/themes/dark
```
::::

## Register a Custom Theme

A custom theme file is a JSON object whose keys are theme names and whose values
are Vega configuration objects. Save this example as `themes.json`:

```{literalinclude} /_examples/themes.json
:language: json
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

The `brand` theme applied to the same chart:

```{vl-chart} /_examples/theme-demo.vl.json
:themes: /_examples/themes.json
:theme: brand
:alt: The stacked bar chart with the brand theme's blue and green palette
```

See {doc}`../advanced/configuration` to set a default theme for every
conversion, and {doc}`../advanced/conversion-overrides` to pass a raw `config`
object instead of a named theme.
