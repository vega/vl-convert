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
let opts = VlOpts {
    format_locale: Some("de-DE".into()),
    time_format_locale: Some("de-DE".into()),
    ..Default::default()
};
```
::::


::::{interface} server
```bash
curl -X POST 'http://localhost:3000/vegalite/svg?format_locale=de-DE' \
  -H 'Content-Type: application/json' \
  --data-binary @chart.vl.json
```
::::
