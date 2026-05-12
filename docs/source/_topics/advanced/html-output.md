---
title: HTML Output
path: advanced/html-output
section: Advanced
order: 400
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# HTML Output Options

HTML output can load Vega dependencies from CDNs or embed the JavaScript bundle.

::::{interface} python
```python
html = vlc.vegalite_to_html(vl_spec, bundle=True)
```
::::

::::{interface} cli
```bash
vl-convert vl2html --bundle --input chart.vl.json --output chart.html
```
::::


::::{interface} rust
```rust
use vl_convert_rs::HtmlOpts;

let html_opts = HtmlOpts { bundle: true, ..Default::default() };
```
::::


::::{interface} server
```bash
curl -X POST http://localhost:3000/vegalite/html \
  -H 'Content-Type: application/json' \
  --data-binary @chart.vl.json > chart.html
```
::::
