---
orphan: true
---

# Experimental static PGFPlots export

**Draft implementation. Native compilation and end-to-end validation are not yet established. This is not a publication-readiness certification.**

The exporter renders Vega/Vega-Lite through the existing SVG pipeline, normalizes that SVG with the existing usvg dependency, and writes vector PGF paths inside a hidden, fixed-size PGFPlots axis. No plotting statistics are reimplemented.

The result is a static picture, **not** a reconstruction of data-aware `\\addplot` commands. Axes, legends, labels, and marks become geometry. Text is outlined; source data, accessible text, interactions, and automatic LaTeX mathematics are not retained. No raster/PDF embedding fallback is used.

## Python

These functions exist only in a build containing this contribution:

```python
from pathlib import Path
import vl_convert as vlc

spec = {
    "data": {"values": [{"x": 1, "y": 2}, {"x": 2, "y": 4}]},
    "mark": "line",
    "encoding": {
        "x": {"field": "x", "type": "quantitative"},
        "y": {"field": "y", "type": "quantitative"},
    },
}
Path("figure.tex").write_text(
    vlc.vegalite_to_pgfplots(spec, standalone=True), encoding="utf-8"
)
```

`vega_to_pgfplots(vg_spec, *, standalone=False)` accepts Vega JSON, and
`svg_to_pgfplots(svg, *, standalone=False)` accepts the restricted SVG subset.
Vega-Lite accepts `vl_version=` to select a bundled compiler. Inputs may be dicts
or JSON strings, except SVG, which is a string. The Python functions are
synchronous. There are no new asyncio or HTTP-server endpoints in this draft.

## Command line

A separate `vl2pgfplots` binary is added to the existing CLI package:

```sh
cargo run -p vl-convert --bin vl2pgfplots -- \
  --input chart.vl.json --output figure.tex --standalone
vl2pgfplots --input-format vega --input chart.vg.json --output figure.tex
vl2pgfplots --input-format svg --standalone < figure.svg > figure.tex
pdflatex -no-shell-escape -halt-on-error figure.tex
```

Input/output default to stdin/stdout. A failed conversion does not overwrite an
existing output file. The new binary uses the converter's default configuration;
it does not duplicate the main CLI's full set of global configuration flags.
`cargo run -p vl-convert` retains its existing default binary.

## Rust

```rust,ignore
use vl_convert_rs::{VlConverter, VlOpts, PgfplotsOpts};

let output = VlConverter::new()
    .vegalite_to_pgfplots(spec, VlOpts::default(), PgfplotsOpts { standalone: true })
    .await?;
std::fs::write("figure.tex", output.tex)?;
```

`vega_to_pgfplots` has the corresponding Vega options. The output retains the SVG
conversion's diagnostic entries and Google Fonts usage metadata. The synchronous
`svg_to_pgfplots` function can also be used without evaluating Vega.

## Supported representation and explicit limits

The implementation is designed for solid-filled vector paths, outlined strokes
(including dashes, caps, and joins), cubic/quadratic curves, affine transforms,
fill opacity, group opacity, nested single-path clips, and outlined glyphs.
Many line/scatter/bar/area and statistical charts reduce to these primitives,
but the native chart-type tests have not yet been run.

Images, gradients (including continuous color legends), patterns, masks, filters,
foreign XML, external references, SVG stylesheets, non-scaling strokes,
non-normal blend modes, and multi-element clip unions fail with an error.
Unsupported features are not silently dropped or rasterized. Even unused
unsupported SVG definitions are rejected by the conservative preflight.

The input limit is 32 MiB, nesting is bounded, and normalized output is limited
to 200,000 painted paths and 2,000,000 emitted segments. Coordinates outside
±20,000 SVG units are rejected to avoid TeX dimension overflow. One SVG pixel
is emitted as 0.75 PDF big points. The viewport is clipped and its physical size
is retained. The output requires PGFPlots 1.18-compatible TeX support.

## Fonts and safety

The registered baseline font database is reused. Font fallback, complex text
shaping, and per-request Google Font overlays require native parity tests before
release. In particular, retaining font-usage metadata is **not** proof that the
outlined glyphs came from the same per-request font environment.

Input strings are never inserted as raw TeX: text becomes paths, and the writer
emits fixed commands plus validated numbers. SVG DTDs, external image resolution,
and external references are rejected. The converter never runs a TeX process.
When compiling generated documents, keep shell escape disabled and sandbox TeX
as with any other typesetting input.

## Validation required before release

The repository's native Rust, Python, and CLI tests must pass, followed by an
end-to-end Vega/Vega-Lite chart corpus comparison against the actual exporter.
Synthetic PGF-emission probes validate the geometry-writing approach only.
They do not validate this native implementation, statistical equivalence,
font resolution, all Vega-Lite constructs, or publication suitability.
