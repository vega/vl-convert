---
title: Native PGFPlots export
path: guides/pgfplots
section: Convert and Export
order: 250
interfaces: [python, cli, rust]
---

<!-- topic-body -->

# Native PGFPlots export

Export Vega-Lite charts as editable PGFPlots axes, data coordinates, legends and
TeX labels. The converter returns TeX source rather than an embedded image.
The bundled Vega-Lite compiler and Vega runtime evaluate the dataflow; PGFPlots
controls typography and layout, which differ from Vega's SVG/PDF output.

::::{interface} python
```python
from pathlib import Path

import vl_convert as vlc

spec = {
    "data": {"values": [{"x": 1, "y": 2}, {"x": 2, "y": 4}]},
    "mark": {"type": "line", "point": True},
    "encoding": {
        "x": {"field": "x", "type": "quantitative"},
        "y": {"field": "y", "type": "quantitative"},
    },
}
tex = vlc.vegalite_to_pgfplots(spec, standalone=True)
Path("figure.tex").write_text(tex, encoding="utf-8")
```

Inputs may also be JSON strings. The asynchronous counterpart is
`await vl_convert.asyncio.vegalite_to_pgfplots(spec, ...)`.
Pass `vl_version="6.4"` to select a bundled compiler instead of the default.
::::

::::{interface} cli
```console
$ vl2pgfplots --input chart.vl.json --output figure.tex --standalone
$ vl2pgfplots --standalone < chart.vl.json > figure.tex
```

`vl2pgfplots` is a separate executable in the CLI package. Use `--vl-version`
to select a bundled compiler and `--vlc-config PATH` to load converter settings,
including resource permissions. Conversion errors leave an existing output file untouched.
::::

::::{interface} rust
```rust
use vl_convert_rs::{PgfplotsOpts, VlConverter};

let spec = std::fs::read_to_string("chart.vl.json")?;
let output = VlConverter::new()
    .vegalite_to_pgfplots(spec, PgfplotsOpts { standalone: true, ..Default::default() })
    .await?;
std::fs::write("figure.tex", output.tex)?;
```

Diagnostics are available in `output.logs`. Use
`vegalite_to_pgfplots_with_version(spec, version, opts)` to select a bundled compiler.
::::

## Typesetting and mathematics

`standalone=True` (CLI: `--standalone`) includes the document class and required
packages. Compile the result with a TeX installation containing PGFPlots:

```sh
pdflatex -no-shell-escape -halt-on-error figure.tex
```

The default is a `tikzpicture` fragment. Its containing document needs `pgfplots`
with `compat=1.18` and the `groupplots`, `fillbetween`, `statistics`, `patchplots`
and `colormaps` libraries. TeX supplies fonts rather than Google Fonts.

Labels are escaped by default. Enable `tex_labels=True` (CLI: `--tex-labels`)
for trusted math labels such as `"$t$ (s)"` or `r"$E_{\mathrm{kin}}$"`.
Raw TeX can read files or execute commands during typesetting; use trusted labels
and a sandboxed typesetter. Disabling shell escape alone is not a sandbox.
The converter itself never runs TeX. Data loading follows the same
resource-access policy as other conversion targets.

## Supported charts and limits

| Area | Support |
| --- | --- |
| Marks | Lines, steps, monotone curves, points, ticks, bars, areas, error bars/bands, box plots, rectangular heatmaps, rules and text |
| Axes | Linear, logarithmic, temporal and categorical; domains, reversal, grids, labels and ticks |
| Composition | Shared-scale layers, two-layer independent y axes, concatenation, simple row/column facets and repeat |
| Data | Inline, named and permission-controlled external sources; static transforms evaluated by the official Vega runtime |

Unsupported mark, guide or layout options raise errors with the property path.
Maps, polar/3D coordinates, interactions, arbitrary custom shapes and deeper
nested composition are not supported. Input must be Vega-Lite, not raw Vega or
SVG. Per-call theme, locale and plugin options are not exposed by this API.

Data coordinates retain their numerical units; display dimensions use 0.75 bp
per pixel. Grouped bars require matching categorical offset and color fields;
negative normalized stacks are rejected. Continuous colorbars interpolate 256
colors from the evaluated scale. Bootstrap intervals and jitter follow Vega's
randomized evaluation; supply precomputed values for identical repeated output.

Limits are 8 MiB serialized input, 100,000 rows per source, 64 panels,
64 layers per panel and 32 MiB TeX output. Converter worker limits still apply.
