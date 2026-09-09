---
title: JavaScript Bundling
path: advanced/javascript-bundling
section: Advanced
order: 460
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# JavaScript Bundling

Most applications should use {doc}`../guides/html-output` to produce a complete
interactive page. Use the bundling API when you are building your own web
integration and need one JavaScript file containing compatible versions of
Vega, Vega-Lite, and Vega Embed.

The standard bundle exposes `vega`, `vegaLite`, and `vegaEmbed` on `window`, so
existing browser code can use them without module imports. Select the same
Vega-Lite version that the browser will render.

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
```console
$ vl-convert bundle-js --vl-version 6.4 --output vega-embed.js
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
```console
$ curl 'http://127.0.0.1:3000/bundling/bundle?vl_version=6.4' \
>   --output vega-embed.js
```

The response carries a `Cache-Control` header that allows caching for one day.
::::

## Add an Application Snippet

A snippet is bundled in the same module scope, so it can refer to `vega`,
`vegaLite`, and `vegaEmbed`. It must not import anything outside the
dependencies bundled with VlConvert.

For example, save this as `snippet.js`:

:::{dropdown} snippet.js
:open:

```{literalinclude} /_examples/bundle-snippet.js
:language: javascript
```
:::

::::{interface} python
```python
with open("snippet.js", encoding="utf-8") as input_file:
    snippet = input_file.read()

bundle = vlc.javascript_bundle(snippet, vl_version="6.4")
```
::::

::::{interface} cli
```console
$ vl-convert bundle-js \
>   --snippet snippet.js \
>   --vl-version 6.4 \
>   --output app-chart.js
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
Save this generated body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/bundle-snippet.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/bundling/bundle-snippet \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output app-chart.js
```
::::

The resulting `app-chart.js` defines the wrapper. This complete page loads the
bundle and calls it with a Vega-Lite specification. Save it as `index.html`
beside `app-chart.js`, then open it in a browser:

:::{dropdown} index.html
:open:

```{literalinclude} /_examples/bundle-demo.html
:language: html
```
:::

Bundling runs build tooling on caller-supplied source. Apply authentication,
body-size limits, and request budgets before exposing the server endpoints to
other users.
