---
title: HTML Output
path: advanced/html-output
section: Advanced
order: 400
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Interactive HTML Output

HTML output creates a web page that displays the chart with Vega Embed. Vega
Embed is the browser helper that loads a Vega or Vega-Lite specification,
creates the Vega view, and provides interactive features such as tooltips and
the action menu.

Use HTML when the reader needs chart interaction. Use SVG, PNG, JPEG, or PDF
when the result should be static.

## Choose How Dependencies Load

By default, the generated page loads Vega, Vega-Lite, and Vega Embed from a
content delivery network (CDN). This produces a smaller file, but the page
needs network access when it opens.

Set `bundle=true` to include those JavaScript dependencies in the HTML file.
Bundled output is larger but can open without fetching the libraries. Data,
images, fonts, and URL-backed plugins can still require network access unless
they are also embedded or otherwise made local.

## Choose a Browser Renderer

The default renderer is `svg`. Use `canvas` when a chart has many marks and
browser performance matters. The `hybrid` renderer can combine both approaches.
This option affects the chart after the HTML page opens in a browser. It does
not change VlConvert's static PNG renderer.

::::{interface} python
```python
import vl_convert as vlc

html = vlc.vegalite_to_html(
    spec,
    bundle=True,
    renderer="svg",
)

with open("chart.html", "w", encoding="utf-8") as output_file:
    output_file.write(html)
```
::::

::::{interface} cli
```bash
vl-convert vl2html \
  --input chart.vl.json \
  --output chart.html \
  --bundle \
  --renderer svg
```
::::

::::{interface} rust
```rust
use vl_convert_rs::{HtmlOpts, Renderer};

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
Put `bundle` and `renderer` beside `spec` in the request body:

```json
{
  "spec": {"mark": "bar", "data": {"values": []}},
  "bundle": true,
  "renderer": "svg"
}
```

Send the body to `POST /vegalite/html` or `POST /vega/html`. The response body
is the generated HTML document.
::::

HTML runs JavaScript in the reader's browser. Apply the same content security
and trust review that you use for other generated web content, especially when
specifications or plugins come from users.
