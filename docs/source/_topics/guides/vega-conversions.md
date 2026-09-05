---
title: Converting Vega
path: guides/vega-conversions
section: Guides
order: 210
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting Vega

Use the Vega functions for charts authored in Vega or already compiled from
Vega-Lite. VlConvert parses and evaluates the specification directly, so the
Vega-Lite options `theme` and `vl_version` do not apply.

Available outputs: SVG, PNG, JPEG, PDF, HTML, a Vega Editor URL, an evaluated
scenegraph, and the fonts VlConvert resolves for the chart.

::::{interface} python
```python
import vl_convert as vlc

svg = vlc.vega_to_svg(vg_spec)
png = vlc.vega_to_png(vg_spec, scale=2)
```

`vg_spec` is a dictionary or JSON string. SVG and HTML functions return text.
PNG, JPEG, and PDF functions return bytes.
::::

::::{interface} cli
```bash
vl-convert vg2svg --input chart.vg.json --output chart.svg
vl-convert vg2png --input chart.vg.json --output chart.png
```

The other commands are `vg2jpeg`, `vg2pdf`, `vg2html`, `vg2url`, `vg2sg`, and
`vg2fonts`.
::::

::::{interface} rust
```rust
use vl_convert_rs::{VgOpts, VlConverter};

let converter = VlConverter::new();
let output = converter
    .vega_to_svg(spec, VgOpts::default(), Default::default())
    .await?;
```

The SVG text is in `output.svg`, and Vega's diagnostic messages are in
`output.logs`.
::::

::::{interface} server
Send a JSON body with the Vega specification in `spec` to the endpoint for the
output you need: `/vega/svg`, `/vega/png`, `/vega/jpeg`, `/vega/pdf`,
`/vega/html`, `/vega/url`, `/vega/scenegraph`, or `/vega/fonts`. All are
`POST` endpoints. The request fields match the Vega-Lite endpoints, minus
`theme` and `vl_version`. See {doc}`../api-reference` for the schemas.
::::

Size, locale, and other per-conversion options work the same way as for
Vega-Lite. See {doc}`image-quality`, {doc}`locales`, and
{doc}`../advanced/conversion-overrides`.
