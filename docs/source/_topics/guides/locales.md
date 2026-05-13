---
title: Locales
path: guides/locales
section: Guides
order: 260
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Locales

Locale overrides are passed to Vega and Vega-Lite's d3-format and
d3-time-format hooks. Use built-in locale names, inline locale JSON, or JSONC
locale files where the surface accepts them.

::::{interface} python
```python
svg = vlc.vegalite_to_svg(
    vl_spec,
    format_locale="de-DE",
    time_format_locale="de-DE",
)
```
::::

::::{interface} cli
```bash
vl-convert vl2svg \
  --format-locale de-DE \
  --time-format-locale de-DE \
  --input chart.vl.json --output chart.svg
```
::::


::::{interface} rust
```rust
use vl_convert_rs::converter::{FormatLocale, TimeFormatLocale, VlOpts};

let opts = VlOpts {
    format_locale: Some(FormatLocale::Name("de-DE".to_string())),
    time_format_locale: Some(TimeFormatLocale::Name("de-DE".to_string())),
    ..Default::default()
};
```
::::


::::{interface} server
```bash
jq -c '{spec: ., format_locale: "de-DE", time_format_locale: "de-DE"}' chart.vl.json |
  curl -X POST http://localhost:3000/vegalite/svg \
  -H 'Content-Type: application/json' \
  --data-binary @-
```
::::
