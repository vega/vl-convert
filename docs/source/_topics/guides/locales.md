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

For persistent default locales and config-file loading, see
{doc}`../advanced/configuration`.

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

let output = converter.vegalite_to_svg(spec, opts, Default::default()).await?;
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

## Locale Objects

Locale objects use the d3-format and d3-time-format locale shapes. For example,
save this d3-format locale as `format-locale.json`:

```json
{
  "decimal": ",",
  "thousands": ".",
  "grouping": [3],
  "currency": ["", " EUR"]
}
```

::::{interface} python
```python
format_locale = {
    "decimal": ",",
    "thousands": ".",
    "grouping": [3],
    "currency": ["", " EUR"],
}

svg = vlc.vegalite_to_svg(vl_spec, format_locale=format_locale)
```
::::

::::{interface} cli
```bash
vl-convert vl2svg \
  --format-locale format-locale.json \
  --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
```rust
use vl_convert_rs::converter::{FormatLocale, VlOpts};

let opts = VlOpts {
    format_locale: Some(FormatLocale::Object(serde_json::json!({
        "decimal": ",",
        "thousands": ".",
        "grouping": [3],
        "currency": ["", " EUR"]
    }))),
    ..Default::default()
};

let output = converter.vegalite_to_svg(spec, opts, Default::default()).await?;
```
::::

::::{interface} server
```bash
jq -c '{
  spec: .,
  format_locale: {
    decimal: ",",
    thousands: ".",
    grouping: [3],
    currency: ["", " EUR"]
  }
}' chart.vl.json |
  curl -X POST http://localhost:3000/vegalite/svg \
  -H 'Content-Type: application/json' \
  --data-binary @-
```
::::

Time format locales use the d3-time-format locale shape, which includes arrays
for days, months, date formats, and time formats. Built-in locale names are
usually simpler unless the application needs a custom locale.
