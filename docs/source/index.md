# VlConvert

VlConvert converts Vega-Lite, Vega, and SVG inputs to static output formats
through Python, CLI, Rust, and HTTP server interfaces.

## Rendered Output Preview

This example starts with one Vega-Lite specification. VlConvert renders the
same chart as SVG, PNG, and PDF. On narrow screens, scroll the preview
horizontally.

<!-- Regenerate the checked-in outputs with `pixi run docs-preview-chart`. -->

:::{container} front-page-chart-scroll
```{image} _static/charts/front-page-chart.svg
:alt: Illustrative chart of sample monthly render counts and p95 latency
:class: front-page-chart-preview
```
:::

{download}`Download PNG output <_static/charts/front-page-chart.png>` ·
{download}`Download PDF output <_static/charts/front-page-chart.pdf>` ·
{download}`Download Vega-Lite spec <_static/charts/front-page-chart.vl.json>`

Choose the documentation root that matches the boundary where you use
VlConvert.

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

```{toctree}
:hidden:
:maxdepth: 2

python/index
cli/index
rust/index
server/index
how-it-works
Changelog <https://github.com/vega/vl-convert/releases>
```
