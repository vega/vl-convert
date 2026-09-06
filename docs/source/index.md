# VlConvert

VlConvert converts Vega-Lite, Vega, and SVG inputs to SVG, PNG, JPEG, PDF, and
HTML. It runs the official Vega and Vega-Lite JavaScript libraries in an
embedded runtime, so it needs no browser or Node.js, and the bundled libraries
need no network access. Network requests occur only for remote data, images,
fonts, or plugins that the input or configuration references.

## Example

Save this Vega-Lite specification, then render it as
`stacked_bar_h.svg` with any VlConvert interface.
The specification loads the barley dataset from the Vega datasets CDN, so
rendering it requires network access.

<!-- Regenerate the checked-in outputs with `pixi run docs-preview-chart`. -->

:::{dropdown} stacked_bar_h.vl.json
:open:

```{literalinclude} _examples/stacked_bar_h.vl.json
:language: json
```
:::

:::::{tab-set}
::::{tab-item} Python
```python
from pathlib import Path

import vl_convert as vlc

spec = Path("stacked_bar_h.vl.json").read_text(encoding="utf-8")
svg = vlc.vegalite_to_svg(spec, vl_version="6.4")
Path("stacked_bar_h.svg").write_text(svg, encoding="utf-8")
```
::::

::::{tab-item} CLI
```bash
vl-convert --vlc-config disabled \
  vl2svg \
  --input stacked_bar_h.vl.json \
  --output stacked_bar_h.svg \
  --vl-version 6.4
```
::::

::::{tab-item} Rust
```rust
use vl_convert_rs::{anyhow, VlConverter, VlOpts, VlVersion};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let spec = std::fs::read_to_string("stacked_bar_h.vl.json")?;
    let converter = VlConverter::new();
    let output = converter
        .vegalite_to_svg(
            spec,
            VlOpts {
                vl_version: VlVersion::v6_4,
                ..Default::default()
            },
            Default::default(),
        )
        .await?;
    std::fs::write("stacked_bar_h.svg", output.svg)?;
    Ok(())
}
```
::::

::::{tab-item} Server
Start the server in one terminal:

```bash
vl-convert serve \
  --vlc-config disabled \
  --port 3000
```

Save this request body:

:::{dropdown} request.json

```{literalinclude} _generated/requests/stacked-bar-h-svg.json
:language: json
```
:::

Send the request from a second terminal:

```bash
curl http://127.0.0.1:3000/vegalite/svg \
  -H 'Content-Type: application/json' \
  --data-binary @request.json \
  --output stacked_bar_h.svg
```
::::
:::::

The generated SVG contains this chart:

```{image} _static/charts/stacked_bar_h.svg
:alt: Horizontal stacked bar chart of barley yield by variety and site
:class: rendered-chart
```

Download the same example in another format, inspect the compiled Vega
specification, or open the chart in the Vega Editor:

:::{container} front-page-download-links
{download}`SVG <_static/charts/stacked_bar_h.svg>` ·
{download}`PNG <_static/charts/stacked_bar_h.png>` ·
{download}`PDF <_static/charts/stacked_bar_h.pdf>` ·
{download}`HTML <_static/charts/stacked_bar_h.html>` ·
{download}`Vega spec <_static/charts/stacked_bar_h.vg.json>` ·
{{ front_page_editor_link }}
:::

## Choose an Interface

The tabs above perform the same conversion through each interface. All four
interfaces share one conversion engine, so a chart renders the same way from
each of them given the same configuration and environment. Pick the interface
that matches how you will call VlConvert.

::::{grid} 1 2 2 4
:gutter: 2

:::{grid-item-card} Python
:link: python/index
:link-type: doc

Use `vl-convert-python` from Python applications and Altair workflows.
:::

:::{grid-item-card} CLI
:link: cli/index
:link-type: doc

Run `vl-convert` from scripts, shells, and build pipelines.
:::

:::{grid-item-card} Rust
:link: rust/index
:link-type: doc

Embed `vl-convert-rs` directly in Rust applications.
:::

:::{grid-item-card} Server
:link: server/index
:link-type: doc

Run `vl-convert serve` as an HTTP rendering worker.
:::
::::

{doc}`how-it-works/rendering` explains the input types, output formats, fonts,
network access, and worker model that every interface shares, and
{doc}`how-it-works/architecture` describes the runtime and crates underneath.

```{toctree}
:hidden:
:maxdepth: 2

python/index
cli/index
rust/index
server/index
how-it-works/index
Changelog <https://github.com/vega/vl-convert/releases>
```
