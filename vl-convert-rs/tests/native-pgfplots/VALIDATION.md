# Native PGFPlots validation

This file describes reproducible acceptance gates and records source-pinned
local evidence. It is not an independent review or a publication certification.
For current build results, use the checks on the PR's exact head commit, not an
older green run. The previous SVG-outline/Python-oracle evidence does not apply.

## CI gates

`.github/workflows/native-pgfplots.yml` runs three read-only jobs:

1. `semantics-and-tex`: both production JavaScript modules, gallery-contract
   tests, actual LaTeX compilation, and analytic PDF checks.
2. `native-interfaces`: Rust and CLI tests, a newly built PyO3 extension, and
   comparisons of original inputs through Node, Rust/Python, and CLI. The TeX
   verifier includes inputs with calculate/window/timeUnit, not just inputs
   already lowered to primitive coordinates.
3. `gallery-conformance`: execute the pinned Altair sources, retain unchanged
   specifications, render official Vega references, compare the built extension
   and CLI, compile native TeX, and reject source/reference failures or regressions
   against `gallery-accepted.json`. No ignored test failures or raster fallback.

The gallery artifact contains original JSON, normalized JSON, IR, official SVG,
TeX from all interfaces, native PDFs, side-by-side PNGs, `index.html`,
`outcomes.json`, and `summary.json`. Randomized calculations execute on all
interfaces but do not claim byte equality across independent random evaluations.

All 188 registry entries must have an outcome. The 48 map/intrinsic-interaction
exclusions remain explicit. Controlled `unsupported` results are distinct from
source, reference, interface, and rendering failures. The accepted baseline
records successful conversion/rendering, **not** full numerical or visual parity.

## Source-pinned local evidence, 2026-11-12

- Emitter SHA-256: `e801256bde81084c69a9e98fee9bc8d7fdcb243d88454d9631e88981a335b54d`.
- Normalizer SHA-256: `650427662de4a44c271796d49aa1e8b40b72987d6324deffa88b307104146d60`.
- Node v22.16.0: **87 emitter tests and 25 official-normalization tests passed**.
- Gallery-contract tests: **3 passed**.
- Actual pdfTeX (TeX Live 2025/dev/Debian): **42 fixtures compiled**, with no
  embedded images, and **16 analytic PDF probes passed**.
- Original-source gallery: 139 exported, 91 converted/rendered, 48 controlled
  unsupported, one source dependency unavailable locally (Polars), 48 excluded.
  CI installs that dependency and fails unless every in-scope source is observed.

PDF probes check linear/log coordinates, interval endpoints, bin bounds/counts,
boxplot outliers, heatmap colors, rolling means, calculated errors, leap-day
calendar coordinates, ticks, variable-size circles, hollow markers/legends,
monotone controls, dual-y alignment, and visible shared axes. Coordinate tolerance
is 0.04 bp; RGB tolerance is 3/255. Circle diameter follows Vega's `sqrt(size)`
convention, not an invented disk-area interpretation.

The original Rust and CLI regressions passed in repair run 43170076899 at
`dabced65362b478f74b4a9af2ff5f3730203d4c6`. Later source changes require the fresh
head CI gates above. Local Node/TeX success is not evidence that native packaging,
all upstream platform checks, or journal-specific figure requirements pass.

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

Gallery acquisition requires Altair commit
`27f89b8d4f2861d446cde8a13480ca5e366eb318` and content-verified dataset files.
The workflow records all source identities and runs `gallery_check.py
--require-native`. A figure still requires review of native TeX typography,
layout, accessibility, and the intended statistical interpretation.
