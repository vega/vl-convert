---
title: Number and Time Locales
path: guides/locales
section: Guides
order: 260
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Number and Time Locales

Locales control how Vega formats values. `format_locale` sets number formatting
such as decimal marks, digit grouping, and currency symbols.
`time_format_locale` sets the names and patterns used for dates and times.

Use a built-in locale name such as `de-DE` or `ja-JP` for standard regional
formats, or pass a locale object for custom rules. The built-in names match the
locale files in [d3-format](https://github.com/d3/d3-format/tree/main/locale)
and [d3-time-format](https://github.com/d3/d3-time-format/tree/main/locale).
Locales change formatting only. They do not change the timezone or the data.

## Use a Built-In Locale

This chart formats revenue with the `$,.0f` pattern and plots dates on the
x axis, so both locale settings affect it:

```{literalinclude} /_examples/locale-demo.vl.json
:language: json
```

::::{interface} python
```python
svg = vlc.vegalite_to_svg(
    spec,
    format_locale="de-DE",
    time_format_locale="de-DE",
)
```

`get_format_locale()` and `get_time_format_locale()` return the definition of
a built-in locale, and {doc}`../api-reference` lists the accepted names.
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
Put the locale names beside `spec` in the request:

```json
{
  "spec": {"mark": "text", "data": {"values": []}},
  "format_locale": "de-DE",
  "time_format_locale": "de-DE"
}
```
::::

Rendered with `de-DE` for both settings, the y axis groups thousands with a
dot and places the euro sign after the number, and the x axis uses German
month names:

```{vl-chart} /_examples/locale-demo.vl.json
:format-locale: de-DE
:time-format-locale: de-DE
:alt: Line chart of monthly revenue with German number and month formatting
```

## Define Custom Number Rules

A number locale follows the
[d3-format locale definition](https://d3js.org/d3-format#locale_format). Save
this example as `format-locale.json`:

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
The option accepts a locale name, an inline JSON object, or a `.json` or
`.jsonc` file:

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
[d3-time-format locale definition](https://d3js.org/d3-time-format#locale_format)
and define day and month names as well as date and time patterns. Prefer a
built-in name when one fits.

See {doc}`../advanced/configuration` to apply a locale to every conversion by
default.
