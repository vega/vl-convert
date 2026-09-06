---
title: Security and Network Access
path: guides/security
section: Guides
order: 280
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Security and Network Access

A Vega or Vega-Lite specification can refer to remote data, local files,
images, fonts, and plugin code. Treat a specification from another user or
system as active input: limit the resources it can reach and the work it can
perform.

VlConvert controls three resource types separately:

| Resource | Control |
| --- | --- |
| Data and images referenced by a specification or SVG | `base_url` and `allowed_base_urls` |
| Google Fonts | Explicit font requests and `auto_google_fonts` |
| Plugin modules and their imports | `vega_plugins` and the plugin domain allowlists |

Allowing one type does not allow the others.

## Restrict Data and Images

By default, `allowed_base_urls` permits any HTTP or HTTPS URL and no local
files for both data and images, and `base_url` resolves relative data and
image URLs against the Vega datasets CDN. Tighten both before processing
untrusted input.

An empty `allowed_base_urls` list blocks every HTTP or HTTPS URL and filesystem
path. Inline `data:` URLs remain allowed. To permit one service, list its URL
prefix. These configuration examples can be applied to any conversion. The CLI
example uses `chart.vl.json` from {doc}`../getting-started/quick-start`:

::::{interface} python
```python
import vl_convert as vlc

vlc.configure(
    base_url=False,
    allowed_base_urls=["https://data.example.com/public/"],
    max_v8_heap_size_mb=512,
    max_v8_execution_time_secs=10,
)
```
::::

::::{interface} cli
```bash
vl-convert \
  --base-url disabled \
  --allowed-base-urls https://data.example.com/public/ \
  --max-v8-heap-size-mb 512 \
  --max-v8-execution-time-secs 10 \
  vl2png --input chart.vl.json --output chart.png
```

`--allowed-base-urls none` blocks every HTTP or HTTPS data and image URL and
every filesystem path. Inline `data:` URLs remain allowed.
::::

::::{interface} rust
```rust
use std::num::NonZeroU64;
use vl_convert_rs::{BaseUrlSetting, VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    base_url: BaseUrlSetting::Disabled,
    allowed_base_urls: vec![
        "https://data.example.com/public/".to_string(),
    ],
    max_v8_heap_size_mb: NonZeroU64::new(512),
    max_v8_execution_time_secs: NonZeroU64::new(10),
    ..Default::default()
})?;
```
::::

::::{interface} server
```bash
vl-convert \
  --base-url disabled \
  --allowed-base-urls https://data.example.com/public/ \
  --max-v8-heap-size-mb 512 \
  --max-v8-execution-time-secs 10 \
  serve \
  --host 127.0.0.1 \
  --port 3000 \
  --opaque-errors
```

`--opaque-errors` keeps internal error details out of client responses. Also
configure authentication, request limits, and render-time budgets before
exposing the listener. See {doc}`/server/deployment`.
::::

Allowlist entries can be URL prefixes, schemes such as `https:`, wildcard
hosts, absolute filesystem directories, or `*`. Use the narrowest prefix that
works, and never use `*` for untrusted specifications. See
{doc}`data-loading` for the full pattern syntax and how URLs are resolved.

## Restrict Fonts and Plugins

Automatic Google Fonts makes network requests based on font names in a
specification. Keep `auto_google_fonts` off unless you need it. If you enable
it, set a variant threshold. See {doc}`fonts`.

Plugins run JavaScript and can import code from allowed domains. Prefer
reviewed local startup plugins, and keep per-request plugins disabled for
untrusted callers. See {doc}`plugins` and {doc}`../advanced/plugin-loading`.

## Limit Resource Use

A specification can consume CPU time or JavaScript memory without loading any
external resource. Set `max_v8_execution_time_secs` and `max_v8_heap_size_mb`
for untrusted workloads. These limits cover only the JavaScript portion of a
conversion, so add process-level memory and time limits as an outer boundary
for a public service. See {doc}`../advanced/memory-management`.
