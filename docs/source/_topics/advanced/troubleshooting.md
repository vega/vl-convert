---
title: Troubleshooting
path: advanced/troubleshooting
section: Advanced
order: 490
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Troubleshooting

Most conversion failures fall into a few categories: invalid specs, blocked
data access, missing fonts, V8 resource limits, or plugin loading errors.

::::{interface} python
Python functions raise exceptions with the underlying converter error message.
Temporarily set stricter diagnostics when investigating font or resource
issues:

```python
vlc.configure(missing_fonts="warn", max_v8_execution_time_secs=10)
```
::::

::::{interface} cli
CLI commands print errors and exit non-zero. Use explicit config flags while
debugging so the active settings are visible in shell history:

```bash
vl-convert --missing-fonts warn --max-v8-execution-time-secs 10 \
  vl2png --input chart.vl.json --output chart.png
```
::::

::::{interface} rust
Rust methods return `Result<_, AnyError>`. The context chain includes messages
from V8 limits, data loading, plugin resolution, and font handling.

```rust
let output = converter
    .vegalite_to_png(spec, Default::default(), Default::default())
    .await?;
```
::::

::::{interface} server
Server errors normally return JSON shaped like `{"error": "..."}`. With
`--opaque-errors=true`, error bodies are empty; disable opaque errors while
debugging trusted staging traffic. If a request fails because a JSON field is
unknown or has the wrong shape, check {doc}`conversion-overrides`.

```bash
vl-convert serve --opaque-errors=false --log-format=json --log-level=debug
```
::::

## V8 Limits

`max_v8_execution_time_secs` terminates JavaScript execution when a conversion
exceeds the configured time. The error message starts with `Conversion timed
out` and includes the configured limit.

`max_v8_heap_size_mb` caps the V8 heap for each worker. When the heap limit is
hit, the error message starts with `V8 heap limit exceeded` and includes worker
memory statistics. Increase the limit, simplify the spec, or omit the limit for
trusted workloads.

After a timeout or heap-limit termination, vl-convert clears the worker's
terminated state so later requests can run. If every request fails, treat it as
a spec/configuration problem rather than a stuck worker.

## Missing Fonts

Set `missing_fonts` to `warn` or `error` when a chart renders with the wrong
font. The default policy falls back silently.

::::{interface} python
```python
vlc.configure(missing_fonts="warn")
fonts = vlc.vegalite_fonts(vl_spec)
```
::::

::::{interface} cli
```bash
vl-convert --missing-fonts warn vl2fonts \
  --input chart.vl.json --output fonts.json
```
::::

::::{interface} rust
```rust
use vl_convert_rs::{VlcConfig, VlConverter};
use vl_convert_rs::converter::MissingFontsPolicy;

let converter = VlConverter::with_config(VlcConfig {
    missing_fonts: MissingFontsPolicy::Warn,
    ..Default::default()
})?;
```
::::

::::{interface} server
```bash
vl-convert serve --missing-fonts warn --log-format=json
```
::::

If `auto_google_fonts` is enabled, fonts that are not local and not in the
Google Fonts catalog are reported through the same missing-font policy.

## Google Fonts

Google Fonts failures can happen during catalog checks, CSS fetches, or font
file downloads. With `missing_fonts="warn"`, catalog and availability problems
are logged as warnings where fallback is possible. With `missing_fonts="error"`,
they fail the conversion.

The Google Fonts cache directory comes from `VL_CONVERT_FONT_CACHE_DIR` when it
is set. Use `VL_CONVERT_FONT_CACHE_DIR=none` to disable the on-disk cache while
debugging cache behavior.

::::{interface} server
Server logs include Google Fonts cache miss and download fields when font work
runs. Public deployments should pair automatic Google Fonts with
`--google-font-variant-threshold` and
`--google-font-cache-miss-penalty-ms`.
::::

## Plugins

Startup plugins are resolved and loaded into workers. A startup plugin that
fails during initialization can poison that worker's Vega initialization; fix
the plugin and reconfigure or restart the converter.

Per-request plugins are disabled by default. Enable them only for trusted
callers and cap ephemeral workers when the server accepts concurrent requests.
HTTP imports inside plugins use plugin import-domain settings, not
`allowed_base_urls`.

When a plugin imports from a CDN, verify both the plugin entry URL and any
redirect/import targets are allowed. For production, pre-bundle multi-file
plugins and pass a local `.js`/`.mjs` file.
