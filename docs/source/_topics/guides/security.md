---
title: Security and Data Access
path: guides/security
section: Guides
order: 270
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Security and Network Access

A Vega or Vega-Lite specification can refer to remote data, local data, images,
fonts, and plugin code. Treat the specification as active input when it comes
from another user or system. Limit both the resources it can reach and the work
it can perform.

VlConvert uses separate controls for three resource types:

| Resource | Primary control |
| --- | --- |
| Data and images referenced by a specification or SVG | `base_url` and `allowed_base_urls` |
| Google Fonts | Explicit font requests and `auto_google_fonts` |
| Plugin modules and their imports | `vega_plugins` and plugin domain allowlists |

Allowing one resource type does not allow the others.

## Restrict Data and Images

By default, `allowed_base_urls` permits HTTP and HTTPS URLs but does not permit
local files. `base_url` supplies the location used to resolve relative data
URLs. Tighten both settings before processing untrusted input.

An empty `allowed_base_urls` list blocks absolute network and filesystem
access. To permit one service, use its full URL prefix:

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

Use `--allowed-base-urls none` to block every absolute data and image URL.
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

Allowlist entries can be URL prefixes, schemes such as `https:`, absolute
filesystem directories, or `*`. Prefer the narrowest prefix that supports the
application. A filesystem entry must name a directory. Do not use `*` for
untrusted specifications.

## Restrict Fonts and Plugins

Automatic Google Fonts can make network requests based on font names in a
specification. Keep `auto_google_fonts` disabled unless this behavior is
required. If you enable it, set a variant threshold and use a cache. See
{doc}`fonts`.

Plugins execute JavaScript and can import code from allowed domains. Prefer
reviewed local startup plugins. Keep per-request plugins disabled when callers
are not trusted. See {doc}`plugins` and
{doc}`../advanced/plugin-loading`.

## Limit Resource Use

An input can require substantial CPU time or JavaScript memory without loading
external resources. Set `max_v8_execution_time_secs` and
`max_v8_heap_size_mb` for untrusted workloads. These limits apply to the
JavaScript portion of a conversion. Use process-level memory and time limits as
an additional boundary for a public service.

See {doc}`../advanced/memory-management` for the performance tradeoffs.
