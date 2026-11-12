---
title: Native PGFPlots export
path: guides/pgfplots
section: Convert and Export
order: 250
interfaces: [python, cli, rust]
---

<!-- topic-body -->

# Native PGFPlots export

This experimental backend emits **editable PGFPlots axes, data coordinates,
legends, and TeX labels**. It replaces the earlier SVG-outline prototype. It does
not reconstruct plots from SVG, hide a picture inside an axis, or embed a PDF.

It is a native emitter for a static Cartesian **subset** of Vega-Lite v5/v6.
The bundled official Vega-Lite compiler and Vega runtime evaluate dataflows;
the native emitter handles axes, series, and TeX. It is not an implementation
of the entire Vega-Lite graphical language. Native
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

Inputs may be dictionaries or JSON strings. `vl_version="v5_8"` selects a
bundled compiler; omitting it uses the default version. The asynchronous counterpart is
`await vl_convert.asyncio.vegalite_to_pgfplots(spec, ...)`, with the same options.

**Security:** `tex_labels=False` escapes TeX metacharacters. Set it to `True`
only for trusted labels: arbitrary TeX can read files or execute commands when
compiled. `-no-shell-escape` alone is not a complete TeX sandbox. Conversion itself never runs TeX. Vega evaluates its own restricted expression
language, not arbitrary caller JavaScript. Data loading uses the same configured
resource-access policy as other converter targets.

## Rust and CLI

```rust
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
TeX process. TeX controls fonts and layout rather than Google Fonts or browser themes.
The existing converter resource policy and worker limits apply. Per-call theme,
locale and plugin options are not exposed by this API. The old draft's `vega_to_pgfplots` and
`svg_to_pgfplots` interfaces are removed: arbitrary Vega/SVG do not retain the
high-level semantics required by this compiler.

`--vl-version` selects a bundled compiler, and `--vlc-config PATH` loads the
existing JSONC converter configuration, including resource permissions. Rust
provides `vegalite_to_pgfplots_with_version(spec, version, opts)`; successful
outputs retain official compiler/runtime diagnostics in `logs`.

A fragment requires `pgfplots` and the `groupplots`, `fillbetween`, `statistics`,
`patchplots`, and `colormaps` libraries. Standalone output includes those imports
and uses compatibility level 1.18. Font size/line width/figure dimensions are
converted from SVG-style pixels to PDF big points at 0.75 bp per pixel; data
coordinates remain in their original numerical units.

## Supported semantics

| Input | Native output and conditions |
|---|---|
| Lines, points, steps, monotone curves | `addplot coordinates`; missing values interrupt paths; monotone controls come from Vega and remain data-coordinate paths |
| Bars | Vertical/horizontal, explicit intervals, zero/normalized stacks; `stack: null` overlaps; grouped bars require the matching categorical `xOffset`/`yOffset` and color field |
| Areas, error bands | Named data-coordinate paths and `fill between`; intervals can be supplied explicitly |
| Error bars | Native asymmetric/symmetric errors on either axis; bounds are retained |
| Box plots | `boxplot prepared`, R7 quartiles, Tukey whiskers/outliers or min–max; default whisker extent 1.5 IQR |
| Heatmaps, colored points | Rectangular patches or scatter metadata, native colorbars; linear quantitative color with viridis or an explicit RGB range |
| Axes and guides | Linear/log, temporal coordinates/calendar labels, categorical ticks, domains, reversal, grids, labels, explicit tick values and Vega numeric formats |
| Tick and variable-size/shape points | Native point/tick marks and size/shape legends; evaluated styles retain source data coordinates |
| Annotations | Rules and editable text at `axis cs` coordinates |
| Composition | Shared-scale layers, two-layer independent y axes, concatenation, simple row/column facets and repeat, native groupplots, explicit scale resolution |
| Data | Inline/named/permission-controlled external sources; official calculate, window, joinaggregate, timeUnit, bin, fold, aggregate and other static dataflow evaluation |

Aggregates include count, sum, mean, min, max, median, quartiles, sample standard
deviation, and standard error. Quantiles use linear interpolation at
`(n-1)*p`. Standard deviation uses denominator `n-1`; standard error divides it
by `sqrt(n)`. Null values are excluded from statistics; absent fields and invalid
numeric types fail. Empty or singleton groups that cannot define a sample
standard error are not assigned an invented interval. Positive/negative stacks
are separated; normalized negative stacks are rejected. Histogram bins are
left-closed/right-open except the final bin, which includes the explicit upper
extent. Extent and step must define an integer number of bins.

Raw confidence-interval composite marks are normalized by the official compiler;
bootstrap intervals are not replaced with standard-error estimates. Randomized
statistics and jitter can vary between evaluations. Supply precomputed intervals
when exact repeated values are required; the exporter does not silently change
Vega's random-number generator or claim seed-independent CI parity.

The normalizer maps each primitive to the compiled mark's real data-source
reference and traces dataflow ancestry to locate generated fields. It does not
assume generated names such as `data_0`, and it rejects ambiguous or unsupported
scopes. Datum-bound styles may be resolved from evaluated marks; plot geometry
is emitted from data values, not traced SVG or scenegraph pixel positions.

Categorical y axes retain Vega's top-to-bottom category order, including explicit
reversal. Continuous colorbars use 256 evaluated RGB knots from the official
scale, including its default palette and reversal; this is a native interpolation
of that palette, not a promise of pixel-identical color sampling. Colors outside
an explicit domain require precomputed styles rather than silent clamping.

## Explicit limitations

Unsupported properties raise path-specific errors. Examples include polar/geographic/3D coordinates, interactions, unsupported
custom shapes or path-dependent styling, unsupported composition/guide options,
and arbitrary JavaScript. Nested data fields are evaluated by Vega and flattened
with collision checks. Unknown graphical options are not silently discarded.
The available static transform set is wider than the native mark/layout set; a
transform succeeding does not guarantee that its resulting chart is supported. Nested composition beyond the
supported layer/panel structure is rejected. Facet/offset variants and color
scale options are deliberately restricted; this is not full grammar coverage.

Limits: 8 MiB serialized input at the Rust/CLI boundary; 100,000 rows per input
source; 64 panels; 64 layers per panel; bounded recursion; 32 MiB TeX output.
Output limits are not a substitute for process memory/time limits. An exported
figure still needs journal-specific typography, accessibility and layout review.

## Reproducible tests

Both production JavaScript modules included by Rust are tested directly under
Node against the same vendored Vega libraries. The test loader makes no network
requests. There is no Python surrogate compiler.

```sh
node --test vl-convert-rs/tests/native-pgfplots/native.test.cjs
node --no-warnings --loader ./vl-convert-rs/tests/native-pgfplots/vendor-loader.mjs \
  --test vl-convert-rs/tests/native-pgfplots/normalize.test.mjs
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

## Gallery conformance (phase 0)

`gallery_export.py` executes pinned Altair example sources and stores their
unchanged emitted specifications. Example datasets are matched against Altair's
recorded Git blob identities. `gallery_run.mjs` preserves the original spec,
reference SVG, normalized spec, native IR, emitted TeX, and diagnostics.
`gallery_render.py` compiles actual TeX and produces side-by-side images. Reports
keep `reference`, `latex`, `numerical_parity`, and `visual_review` separate:
LaTeX success alone is not a scientific-fidelity pass.

The source pin is Altair `27f89b8d4f2861d446cde8a13480ca5e366eb318`.
External-source acquisition failures are recorded separately from compiler
failures. See the workflow artifact and `pgfplots-conformance.json` for observed
results and unresolved cases; the earlier gallery registry is a planning
classification, not a list of verified passes.


## Acceptance and limitations

The [gallery coverage record](pgfplots-gallery.md) separates the original
planning classifications from executed conformance results. The CI artifact
`altair-pgfplots-conformance` includes all original specifications, reference
SVGs, generated TeX/PDF pairs, per-stage diagnostics, and a summary. Controlled
unsupported cases are reported; source/reference failures and regressions in
previously accepted examples fail the job.

Successful conversion is not a claim of pixel-identical output or independent
scientific review. The normalization fixtures separately measure temporal and
logarithmic coordinates, rolling means, calculated uncertainties, physical tick
lengths, symbol areas, and independent-axis alignment. Native Python and CLI
output must agree with the production JavaScript modules for deterministic
fixtures. Randomized bootstrap/jitter cases are identified rather than given a
false byte-equivalence claim.
