---
title: JavaScript Bundling
path: advanced/javascript-bundling
section: Advanced
order: 470
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# JavaScript Bundling

Most applications should use {doc}`html-output` to create a complete
interactive page. Use the bundling API only when you are building your own web
integration and need a JavaScript asset that contains compatible versions of
Vega, Vega-Lite, and Vega Embed.

Vega Embed is the browser helper that creates a Vega view from a specification.
The standard bundle exposes `vega`, `vegaLite`, and `vegaEmbed` on `window` so
existing browser code can use them without additional module imports.

Select the same Vega-Lite version used by the specifications that the browser
will render.

## Get the Standard Bundle

::::{interface} python
```python
import vl_convert as vlc

bundle = vlc.javascript_bundle(vl_version="6.4")
with open("vega-embed.js", "w", encoding="utf-8") as output_file:
    output_file.write(bundle)
```
::::

::::{interface} cli
```bash
vl-convert bundle-js --vl-version 6.4 --output vega-embed.js
```
::::

::::{interface} rust
```rust
use vl_convert_rs::{VlConverter, VlVersion};

let converter = VlConverter::new();
let bundle = converter
    .get_vegaembed_bundle(VlVersion::v6_4)
    .await?;

std::fs::write("vega-embed.js", bundle)?;
```
::::

::::{interface} server
```bash
curl 'http://127.0.0.1:3000/bundling/bundle?vl_version=6.4' \
  --output vega-embed.js
```

The response is cacheable JavaScript.
::::

## Add an Application Snippet

A custom snippet is bundled in the same module scope, so it can refer to
`vega`, `vegaLite`, or `vegaEmbed`. It must not contain imports outside the
dependencies bundled with VlConvert.

For example, save this as `snippet.js`:

```javascript
window.renderVegaLite = (element, spec, options = {}) =>
  vegaEmbed(element, spec, options)
```

::::{interface} python
```python
with open("snippet.js", encoding="utf-8") as input_file:
    snippet = input_file.read()

bundle = vlc.javascript_bundle(snippet, vl_version="6.4")
```
::::

::::{interface} cli
```bash
vl-convert bundle-js \
  --snippet snippet.js \
  --vl-version 6.4 \
  --output app-chart.js
```
::::

::::{interface} rust
```rust
let bundle = converter
    .bundle_vega_snippet(
        include_str!("../snippet.js"),
        VlVersion::v6_4,
    )
    .await?;
```
::::

::::{interface} server
```bash
curl http://127.0.0.1:3000/bundling/bundle-snippet \
  -H 'Content-Type: application/json' \
  --data '{"snippet":"window.ready = Boolean(vegaEmbed)","vl_version":"6.4"}' \
  --output app-chart.js
```
::::

Bundling executes build work on input source. Apply authentication, body-size
limits, and request budgets when exposing the server endpoints to other users.
