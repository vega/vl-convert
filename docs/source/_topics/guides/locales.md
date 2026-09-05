---
title: Number and Time Locales
path: guides/locales
section: Guides
order: 260
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Number and Time Locales

Locales change how Vega and Vega-Lite format values. `format_locale` controls
numbers, including decimal marks, grouping, and currency. `time_format_locale`
controls names and patterns for dates and times.

Use a built-in locale name for common regional formats. Use a locale object
when the application needs custom rules. Locale settings change formatting,
not the timezone or the underlying data values.

## Use a Built-In Locale

::::{interface} python
```python
svg = vlc.vegalite_to_svg(
    spec,
    format_locale="de-DE",
    time_format_locale="de-DE",
)
```
::::

::::{interface} cli
```bash
vl-convert vl2svg \
  --input chart.vl.json --output chart.svg \
  --format-locale de-DE \
  --time-format-locale de-DE
```
::::

::::{interface} rust
```rust
use vl_convert_rs::converter::{FormatLocale, TimeFormatLocale};
use vl_convert_rs::VlOpts;

let options = VlOpts {
    format_locale: Some(FormatLocale::Name("de-DE".to_string())),
    time_format_locale: Some(TimeFormatLocale::Name("de-DE".to_string())),
    ..Default::default()
};

let output = converter
    .vegalite_to_svg(spec, options, Default::default())
    .await?;
```
::::

::::{interface} server
Put locale names beside `spec` in the request:

```json
{
  "spec": {"mark": "text", "data": {"values": []}},
  "format_locale": "de-DE",
  "time_format_locale": "de-DE"
}
```
::::

## Define Custom Number Rules

A number locale follows the
[d3-format locale definition](https://d3js.org/d3-format#locale_format).
Save this example as `format-locale.json`:

```json
{
  "decimal": ",",
  "thousands": ".",
  "grouping": [3],
  "currency": ["", " EUR"]
}
```

::::{interface} python
Pass the equivalent dictionary:

```python
format_locale = {
    "decimal": ",",
    "thousands": ".",
    "grouping": [3],
    "currency": ["", " EUR"],
}

svg = vlc.vegalite_to_svg(spec, format_locale=format_locale)
```
::::

::::{interface} cli
Pass a locale name, inline JSON object, or `.json` or `.jsonc` file:

```bash
vl-convert vl2svg \
  --format-locale format-locale.json \
  --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
```rust
use vl_convert_rs::converter::FormatLocale;
use vl_convert_rs::{serde_json, VlOpts};

let options = VlOpts {
    format_locale: Some(FormatLocale::Object(serde_json::json!({
        "decimal": ",",
        "thousands": ".",
        "grouping": [3],
        "currency": ["", " EUR"]
    }))),
    ..Default::default()
};
```
::::

::::{interface} server
The request field accepts the locale object directly:

```json
{
  "spec": {"mark": "text", "data": {"values": []}},
  "format_locale": {
    "decimal": ",",
    "thousands": ".",
    "grouping": [3],
    "currency": ["", " EUR"]
  }
}
```
::::

Custom time locales follow the
[d3-time-format locale definition](https://d3js.org/d3-time-format#locale_format).
They define names for days and months as well as date and time patterns.
Built-in names are shorter and less error-prone when a standard locale fits.

See {doc}`../advanced/configuration` to apply locale defaults to every
conversion.
