---
title: HTML Output
path: guides/html-output
section: Guides
order: 235
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Interactive HTML Output

HTML output is a web page that renders the chart with Vega Embed, the browser
helper that loads a Vega or Vega-Lite specification, creates the Vega view, and
adds tooltips and the action menu. Use it when the reader needs interaction.
Use SVG, PNG, JPEG, or PDF for static results.

## Choose How Dependencies Load

By default the page loads Vega, Vega-Lite, and Vega Embed from a content
delivery network (CDN). The file stays small, but the page needs network access
when it opens.

Set `bundle=true` to include those libraries in the file, along with any
plugins and the fonts VlConvert resolved for the chart. The page then opens
offline, although data and images referenced by URL still need network access,
as do fonts that were not embedded.

## Choose a Browser Renderer

`renderer` selects how the browser draws the chart: `svg` (the default),
`canvas` for charts with many marks where browser performance matters, or
`hybrid`, which draws marks on canvas and text as SVG. This option only affects
the page in the browser. It does not change how VlConvert renders PNG.

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

Send it to `POST /vegalite/html` or `POST /vega/html`. The response body is the
HTML document.
::::

HTML runs JavaScript in the reader's browser. Review it like any other
generated web content, especially when specifications or plugins come from
users. See {doc}`../advanced/javascript-bundling` to build the browser bundle
without an HTML page, and {doc}`../advanced/plugin-loading` for how plugins are
included in HTML output.
