# VlConvert

VlConvert converts Vega-Lite, Vega, and SVG inputs to SVG, PNG, JPEG, PDF, and
HTML. It runs the official Vega and Vega-Lite JavaScript libraries in an
embedded runtime, so it needs no browser or Node.js, and the bundled libraries
need no network access. Network requests occur only for remote data, images,
fonts, or plugins that the input or configuration references.

## Example

A Vega-Lite specification and the SVG that VlConvert renders from it. The
links below download the SVG, PNG, PDF, bundled HTML, and compiled Vega
outputs, or open the chart in the Vega Editor.

<!-- Regenerate the checked-in outputs with `pixi run docs-preview-chart`. -->

:::{dropdown} front-page-chart.vl.json
:open:

```{literalinclude} _static/charts/front-page-chart.vl.json
:language: json
```
:::

```{image} _static/charts/front-page-chart.svg
:alt: Bar chart of charts rendered per month, from January to June
:class: rendered-chart
```

:::{container} front-page-download-links
{download}`SVG <_static/charts/front-page-chart.svg>` ·
{download}`PNG <_static/charts/front-page-chart.png>` ·
{download}`PDF <_static/charts/front-page-chart.pdf>` ·
{download}`HTML <_static/charts/front-page-chart.html>` ·
{download}`Vega spec <_static/charts/front-page-chart.vg.json>` ·
{{ front_page_editor_link }}
:::

## Choose an Interface

All four interfaces share one conversion engine, so a chart renders the same
way from each of them given the same configuration and environment. Pick the
one that matches how you will call VlConvert.

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
