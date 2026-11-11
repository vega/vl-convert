# Native PGFPlots validation record

Recorded 2026-11-11. This is automated evidence and author self-review, not an
independent review or a publication-quality certification.

## Exact source tested

- File: `vl-convert-rs/src/js/native_pgfplots.js`
- Git blob: `d5203a29fadc8ab4916c5c05cb6a4e593e85538f`
- SHA-256: `fe76a3733733a11a97c242a5be46098ef18064c5e205be6578486083019e0fc1`
- Node: `v22.16.0`
- TeX: `pdfTeX 3.141592653-2.6-1.40.26 (TeX Live 2025/dev/Debian)`

The exact compiler source embedded by Rust was executed under Node. The TeX
renderings were emitted by that source, not a Python stand-in.

## Executed successfully

- `node --test --experimental-test-coverage vl-convert-rs/tests/native-pgfplots/native.test.cjs`: **80 passed, 0 failed, 0 skipped**.
- V8 coverage of the compiler module: **93.02% lines, 73.08% branches, 85.43% functions**. These figures are not full coverage and not a correctness proof.
- `python vl-convert-rs/tests/native-pgfplots/verify_tex.py OUTPUT`: **31 fixtures compiled successfully with real pdflatex**; PDFs had one page and no embedded images.
- **7 numerical/rendering probes passed**: linear coordinates, log coordinates, explicit uncertainty endpoints, asymmetric intervals, histogram bounds/counts, a horizontal-boxplot outlier, and quantitative heatmap colors.
- Coordinate probes use a **0.04 bp** tolerance. Heatmap samples use an RGB tolerance of **3/255**.
- Generated PNGs for facets, heatmaps, and horizontal box plots were visually inspected.

Fixes discovered while running these tests included last-bin histogram clipping,
horizontal-boxplot outlier placement, unintended errorbar center markers, and
confusing overlapping bars (`stack: null`) with explicitly offset/grouped bars.

## Not verified in this environment

No Rust compiler or dependency-download access was available. Consequently the
Rust wrapper, patched PyO3 extension (sync/async), CLI binary, full upstream CI,
Rust formatting/lints, documentation build, and cross-platform packaging were
**not executed**. Their test sources are included. The rendering driver supports
`--native` and `--cli PATH` to compare built interfaces against the shared source;
those flags were not used for the results above.

No comparison against a live Vega reference renderer was executed. Numerical
probes independently check selected invariants; TeX typography and layout are
intentionally native, not promised to be pixel-identical to Vega. Supported
input is a documented subset, not the entire Vega-Lite grammar. Keep the PR in
draft until native integration and upstream CI pass and the API scope is reviewed.

## Follow-up numerical regression run

The original evidence above remains a historical record for its pinned source.
New regressions were first observed failing, then passed after fixes for decimal
histogram roundoff, single-series normalization, invalid explicit stacks,
aggregate field collisions, and object-property-order-independent scale comparison.
Current compiler SHA-256: `9f8c43bb856987af72fa922fba9845a91956b6a3d047e3b04b1940fae7613c93`.
The exact updated source passed **87 Node tests** in GitHub Actions.
Two additional TeX fixtures cover decimal bins and single-series normalization;
their rendering results must be checked in the Native PGFPlots workflow.
