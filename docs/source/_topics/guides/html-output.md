---
title: HTML Output
path: guides/html-output
section: Guides
order: 235
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Interactive HTML Output

HTML output is a web page that renders the chart with Vega Embed, the browser helper that loads a Vega or Vega-Lite specification, creates the Vega view, and adds tooltips and the action menu. Use it when the reader needs interaction. Use SVG, PNG, JPEG, or PDF for static results.

## Choose How Dependencies Load

By default the page loads Vega, Vega-Lite, and Vega Embed from a content delivery network (CDN). The file stays small, but the page needs network access when it opens.

Set `bundle=true` to include those libraries in the file, along with any plugins and the fonts VlConvert resolved for the chart. The browser no longer needs to fetch these dependencies. Data and images referenced by URL, and fonts that were not embedded, may still need network access.

HTML generation normally embeds the specification without evaluating it. When Google Font discovery, an explicit Google Font request, or local font embedding is enabled, VlConvert evaluates the chart to resolve fonts. That evaluation can load external data and images under VlConvert's access policy before the browser loads them again.

## Choose a Browser Renderer

`renderer` selects how the browser draws the chart: `svg` (the default), `canvas` for charts with many marks where browser performance matters, or `hybrid`, which draws marks on canvas and text as SVG. This option only affects the page in the browser. It does not change how VlConvert renders PNG.

The examples use the input from Quick Start:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/quick-start.vl.json
:language: json
```
:::

::::{interface} python
```python
from pathlib import Path

import vl_convert as vlc

spec = Path("chart.vl.json").read_text(encoding="utf-8")
html = vlc.vegalite_to_html(
    spec,
    bundle=True,
    renderer="svg",
)

Path("chart.html").write_text(html, encoding="utf-8")
```
::::

::::{interface} cli
```console
$ vl-convert vl2html \
>   --input chart.vl.json \
>   --output chart.html \
>   --bundle \
>   --renderer svg
```
::::

::::{interface} rust
```rust
use vl_convert_rs::{HtmlOpts, Renderer, VlConverter};

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::new();
let output = converter
    .vegalite_to_html(
        spec,
        Default::default(),
        HtmlOpts {
            bundle: true,
            renderer: Renderer::Svg,
        },
    )
    .await?;

std::fs::write("chart.html", output.html)?;
```
::::

::::{interface} server
Put `bundle` and `renderer` beside `spec` in the request body. Save this complete body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/html-bundled.json
:language: json
```
:::

Send it to `POST /vegalite/html`:

```console
$ curl http://127.0.0.1:3000/vegalite/html \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.html
```

The response body is the HTML document. Use `POST /vega/html` for direct Vega input.
::::

HTML runs JavaScript in the reader's browser. Review it like any other generated web content, especially when specifications or plugins come from users. See {doc}`../advanced/javascript-bundling` to build the browser bundle without an HTML page, and {doc}`../advanced/plugin-loading` for how plugins are included in HTML output.
