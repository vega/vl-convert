# VlConvert

VlConvert converts Vega-Lite, Vega, and SVG inputs to SVG, PNG, JPEG, PDF, and
HTML. It runs the official Vega and Vega-Lite JavaScript libraries in an
embedded runtime, so it needs no browser or Node.js, and the bundled libraries
need no network access. Network requests occur only for remote data, images,
fonts, or plugins that the input or configuration references.

## Rendered Output Preview

One Vega-Lite specification, rendered by VlConvert as SVG, PNG, and PDF.

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
