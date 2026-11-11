---
orphan: true
---

# Native PGFPlots export

This experimental backend emits **editable PGFPlots axes, data coordinates,
legends, and TeX labels**. It replaces the earlier SVG-outline prototype. It does
not reconstruct plots from SVG, hide a picture inside an axis, or embed a PDF.

It is a strict, independent compiler for a static Cartesian **subset** of
Vega-Lite v5/v6, not an implementation of the entire Vega-Lite language. Native
TeX typography, tick selection, layout, and default padding differ from Vega.
Numerical meaning, rather than pixel-identical appearance, is the contract.

## Python

```python
from pathlib import Path
import vl_convert as vlc

spec = {
    "data": {"values": [{"t": 1, "E": 2}, {"t": 2, "E": 4}]},
    "mark": {"type": "line", "point": True},
    "encoding": {
        "x": {"field": "t", "type": "quantitative", "title": "$t$ (s)"},
        "y": {"field": "E", "type": "quantitative", "title": r"$E_{\mathrm{kin}}$ (eV)"},
    },
}
Path("figure.tex").write_text(
    vlc.vegalite_to_pgfplots(spec, standalone=True, tex_labels=True),
    encoding="utf-8",
)
```

Inputs may be dictionaries or JSON strings. The asynchronous counterpart is
`await vl_convert.asyncio.vegalite_to_pgfplots(spec, ...)`, with the same options.

**Security:** `tex_labels=False` escapes TeX metacharacters. Set it to `True`
only for trusted labels: arbitrary TeX can read files or execute commands when
compiled. `-no-shell-escape` alone is not a complete TeX sandbox. Conversion
itself never runs TeX, evaluates supplied expressions, or loads remote data.

## Rust and CLI

```rust,ignore
use vl_convert_rs::{PgfplotsOpts, VlConverter};

let output = VlConverter::new().vegalite_to_pgfplots(
    spec,
    PgfplotsOpts { standalone: true, tex_labels: false },
).await?;
std::fs::write("figure.tex", output.tex)?;
```

```sh
cargo run -p vl-convert --bin vl2pgfplots -- \
  --input chart.vl.json --output figure.tex --standalone
vl2pgfplots --standalone < chart.vl.json > figure.tex
pdflatex -no-shell-escape -halt-on-error figure.tex
```

`--tex-labels` enables trusted mathematical labels. Conversion errors leave an
existing output file untouched. The compiler returns TeX; it never launches a
TeX process. Browser themes, Google Fonts, plugins, locales and `VlOpts` do not
apply to this independent backend. The old draft's `vega_to_pgfplots` and
`svg_to_pgfplots` interfaces are removed: arbitrary Vega/SVG do not retain the
high-level semantics required by this compiler.

A fragment requires `pgfplots` and the `groupplots`, `fillbetween`, `statistics`,
`patchplots`, and `colormaps` libraries. Standalone output includes those imports
and uses compatibility level 1.18. Font size/line width/figure dimensions are
converted from SVG-style pixels to PDF big points at 0.75 bp per pixel; data
coordinates remain in their original numerical units.

## Supported semantics

| Input | Native output and conditions |
|---|---|
| Lines, points, steps | `addplot coordinates`; missing values interrupt paths |
| Bars | Vertical/horizontal, explicit intervals, zero/normalized stacks; `stack: null` overlaps; grouped bars require the matching categorical `xOffset`/`yOffset` and color field |
| Areas, error bands | Named data-coordinate paths and `fill between`; intervals can be supplied explicitly |
| Error bars | Native asymmetric/symmetric errors on either axis; bounds are retained |
| Box plots | `boxplot prepared`, R7 quartiles, Tukey whiskers/outliers or min–max; default whisker extent 1.5 IQR |
| Heatmaps, colored points | Rectangular patches or scatter metadata, native colorbars; linear quantitative color with viridis or an explicit RGB range |
| Axes and guides | Linear/log, categorical ticks, domains, reversal, grids, labels, explicit tick values, `.nf`/`.ne` numerical formatting |
| Annotations | Rules and editable text at `axis cs` coordinates |
| Composition | Shared-scale layers, concatenation, simple row/column facets and repeat, native groupplots, explicit scale resolution |
| Data | Inline values/named inline datasets; structured filters, aggregate, fold, explicit-step/extent histograms |

Aggregates include count, sum, mean, min, max, median, quartiles, sample standard
deviation, and standard error. Quantiles use linear interpolation at
`(n-1)*p`. Standard deviation uses denominator `n-1`; standard error divides it
by `sqrt(n)`. Null values are excluded from statistics; absent fields and invalid
numeric types fail. Empty or singleton groups that cannot define a sample
standard error are not assigned an invented interval. Positive/negative stacks
are separated; normalized negative stacks are rejected. Histogram bins are
left-closed/right-open except the final bin, which includes the explicit upper
extent. Extent and step must define an integer number of bins.

For reproducibility, **bootstrap confidence intervals are not recomputed**.
Supply precomputed lower/upper bounds. A raw `errorband` whose default implies a
confidence interval is rejected, not silently replaced with standard error.

## Explicit limitations

Unsupported properties raise path-specific errors. Examples include temporal,
polar/geographic/3D coordinates, external URLs, freeform expressions, arbitrary
Vega signals and interactions, nested data-field paths, arbitrary interpolation,
custom stack-order fields, and most browser-specific styling. Materialize
unsupported transformations before export. Nested composition beyond the
supported layer/panel structure is rejected. Facet/offset variants and color
scale options are deliberately restricted; this is not full grammar coverage.

Limits: 8 MiB serialized input at the Rust/CLI boundary; 100,000 rows per input
source; 64 panels; 64 layers per panel; bounded recursion; 32 MiB TeX output.
Output limits are not a substitute for process memory/time limits. An exported
figure still needs journal-specific typography, accessibility and layout review.

## Reproducible tests

The JavaScript source included by Rust is the same source tested by Node; there
is no Python surrogate implementation.

```sh
node --test vl-convert-rs/tests/native-pgfplots/native.test.cjs
python vl-convert-rs/tests/native-pgfplots/verify_tex.py /tmp/pgfplots-check
cargo test -p vl-convert-rs --test test_pgfplots -- --test-threads=1
cargo test -p vl-convert --test test_pgfplots_cli -- --test-threads=1
pixi run dev-py
pixi run test-py
```

The rendering driver requires Node 22, TeX Live/PGFPlots and PyMuPDF. It compiles
all fixtures, checks that PDFs contain vector graphics and text, and independently
measures linear/log coordinates, uncertainty endpoints, histogram boundaries,
boxplot outliers and quantitative colors. It also writes PNGs for visual review.
Use `--native` with the built extension and `--cli /path/to/vl2pgfplots` to compare
those interfaces byte-for-byte with the shared compiler output. Missing native
components fail these explicit checks rather than being silently skipped.

The fork PR records which tests were actually run. Passing Node/TeX tests alone
does not establish that Rust/PyO3 packaging builds, that every Vega-Lite feature
is implemented, or that a figure meets a publication's requirements.
