# Native PGFPlots validation

This document defines reproducible acceptance gates, not an independent review or
publication certification. Use checks on the PR's exact head commit for current
build status. Previous SVG-outline/Python-oracle results do not apply.

## Persistent CI gates

`.github/workflows/native-pgfplots.yml` runs three read-only jobs:

1. `semantics-and-tex`: both production JavaScript modules against the bundled
   official libraries, gallery-contract tests, real LaTeX compilation, and
   analytic PDF geometry/color probes.
2. `native-interfaces`: Rust/CLI tests, a newly built PyO3 extension, and original
   calculate/window/timeUnit inputs through Node, Python and CLI. The verifier
   renders the actual extension output, not only pre-normalized fixtures.
3. `gallery-conformance`: execute pinned Altair sources, preserve original specs,
   render official references, compare the built Python and CLI outputs, compile
   native TeX, and reject regressions against `gallery-accepted.json`.

All 188 catalog entries require an observed outcome. The 48 map/intrinsic-
interaction exclusions are explicit. Source/reference/interface/rendering errors
fail the gate; controlled unsupported results remain distinct. The baseline
currently requires 91 original examples to convert and render. It does not
certify complete numerical/visual parity for every example.

The gallery artifact contains original/normalized JSON, native IR, official SVG,
reference PNGs from the **built Vega renderer**, TeX from all interfaces, native
PDFs, paired PNGs, `index.html`, `outcomes.json`, and `summary.json`. Generic SVG
rasterizers are not used to approximate reference gradients or fonts.

## Numeric comparison contract

For deterministic specifications the built Python and CLI output must be
byte-identical. Node may use a different V8/libm version. Differences are allowed
**only inside coordinate blocks**, with absolute and relative tolerance 1e-12.
All other TeX, styles, labels, axes, point counts, and path structure remain
exact. Observed maximum differences are recorded; production coordinates are
never rounded to force a pass. Negative tests reject changed styles, point
ordering, nonfinite values and meaningful numerical differences.

Four deterministic examples exposed runtime roundoff at the previous source
revision: density_repeat, density_stack, line_chart_with_generator, and
poly_fit_regression. Their maximum observed coordinate difference was
1.438849039914203e-13; PyO3 and CLI were byte-identical. Independently randomized
statistics/jitter execute on all interfaces but make no byte-equality claim.

## Source-pinned local evidence, 2026-11-12

- Emitter SHA-256: `70746c030c4db125deba72ba668da92b00adba30c87e02e1a25c38893f3b719f`.
- Normalizer SHA-256: `2285a933ec89c52070eab05638bd5c57c2c0d932a0e5b2c87d3b9380ab440058`.
- Node v22.16.0: **87 emitter and 29 normalization tests passed**.
- Gallery-contract tests: **4 passed**.
- pdfTeX (TeX Live 2025/dev/Debian): **43 fixtures compiled and 17 analytic PDF
  probes passed**, without embedded images.

Probes cover linear/log/calendar coordinates, uncertainty bounds, bins, boxplot
outliers, explicit/default color scales, categorical order, rolling means, ticks,
variable sizes, hollow markers/legends, monotone curves, dual axes and shared
axes. Coordinate tolerance is 0.04 bp; RGB tolerance is 3/255. Circle diameter is
`sqrt(size)`, matching Vega. Continuous palettes use 256 evaluated RGB knots;
this is native interpolation, not pixel-identical graphics.

Earlier source `f7b05b4` passed actual Rust/Python/CLI comparison and 42 TeX
fixtures in run 43178366956. Later changes require fresh head checks; local
Node/TeX results do not prove native packaging or all-platform CI succeeds.

## Reproduce

```sh
node --test vl-convert-rs/tests/native-pgfplots/native.test.cjs
node --no-warnings --loader ./vl-convert-rs/tests/native-pgfplots/vendor-loader.mjs --test vl-convert-rs/tests/native-pgfplots/normalize.test.mjs
python vl-convert-rs/tests/native-pgfplots/test_gallery_contracts.py
python vl-convert-rs/tests/native-pgfplots/verify_tex.py /tmp/pgfplots
cargo test -p vl-convert-rs --test test_pgfplots -- --test-threads=1
cargo test -p vl-convert --test test_pgfplots_cli -- --test-threads=1
pixi run dev-py
pixi run test-py
python vl-convert-rs/tests/native-pgfplots/verify_tex.py /tmp/pgfplots-native --native --cli target/debug/vl2pgfplots
```

Gallery acquisition pins Altair commit
`27f89b8d4f2861d446cde8a13480ca5e366eb318` and verifies dataset contents. CI runs
`gallery_check.py --require-native`. Native TeX typography, layout, accessibility
and the intended statistical interpretation still require figure-specific review.
