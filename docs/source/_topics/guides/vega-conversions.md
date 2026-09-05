---
title: Converting Vega
path: guides/vega-conversions
section: Guides
order: 210
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting Vega

Use a Vega input when the chart was authored in Vega or was already compiled
from Vega-Lite. VlConvert evaluates the Vega specification directly, so
Vega-Lite themes and compiler-version options do not apply.

Available results include SVG, PNG, JPEG, PDF, HTML, a Vega Editor URL, and an
evaluated scenegraph. Use the Vega-Lite conversion API instead if you need
VlConvert to compile the input first.

::::{interface} python
```python
import vl_convert as vlc

svg = vlc.vega_to_svg(vg_spec)
png = vlc.vega_to_png(vg_spec, scale=2)
```

`vg_spec` can be a Python dictionary or a JSON string. SVG functions return
text, while raster and PDF functions return bytes.
::::

::::{interface} cli
```bash
vl-convert vg2svg --input chart.vg.json --output chart.svg
vl-convert vg2png --input chart.vg.json --output chart.png
```

The `vg2jpeg`, `vg2pdf`, `vg2html`, `vg2url`, and `vg2sg`
commands provide the other output types.
::::

::::{interface} rust
```rust
use vl_convert_rs::{VgOpts, VlConverter};

let converter = VlConverter::new();
let output = converter
    .vega_to_svg(spec, VgOpts::default(), Default::default())
    .await?;
```

The SVG text is in `output.svg`, and Vega diagnostic messages are in
`output.logs`.
::::

::::{interface} server
Send a JSON body with the Vega value in `spec` to the endpoint for the
required output. For example, `POST /vega/png` returns PNG bytes and
`POST /vega/svg` returns SVG text.

The other endpoints are `/vega/jpeg`, `/vega/pdf`, `/vega/html`,
`/vega/url`, and `/vega/scenegraph`. See {doc}`../api-reference` for
complete request and response schemas.
::::
