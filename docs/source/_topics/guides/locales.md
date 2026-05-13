---
title: Locales
path: guides/locales
section: Guides
order: 260
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Locales

Pass d3-format and d3-time-format locale names or locale JSON where the
surface accepts locale overrides.

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
