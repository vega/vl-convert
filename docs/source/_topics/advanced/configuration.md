---
title: Configuration
path: advanced/configuration
section: Advanced
order: 405
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Configuration

vl-convert has one shared converter configuration model. The surfaces differ
mostly in how that configuration is supplied and when it can change.

::::{interface} python
`configure()` patches process-wide defaults for later conversions. `load_config()`
loads the same JSONC config file format used by the CLI and replaces the active
process configuration. Per-call arguments such as `scale`, `theme`,
`format_locale`, and `google_fonts` override process defaults for that call
where those arguments exist.

```python
import vl_convert as vlc

vlc.configure(
    allowed_base_urls=[],
    auto_google_fonts=True,
    google_font_variant_threshold=16,
    max_v8_heap_size_mb=1024,
    max_v8_execution_time_secs=10,
)
```
::::

::::{interface} cli
The CLI starts from library defaults, optionally loads a JSONC config file, then
applies environment variables and explicit flags:

```text
CLI flags > VLC_* environment variables > --vlc-config/default config file > library defaults
```

`--vlc-config disabled` skips config-file loading. When `--vlc-config` is
omitted, vl-convert loads the platform default config path if it exists.

```bash
VLC_AUTO_GOOGLE_FONTS=true \
vl-convert --google-font-variant-threshold 16 \
  vl2png --input chart.vl.json --output chart.png
```
::::

::::{interface} rust
Rust callers construct `VlcConfig` directly and pass it to `VlConverter`.
There is no environment-variable or config-file precedence unless the
application implements one.

```rust
use std::num::NonZeroU64;
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    allowed_base_urls: Vec::new(),
    auto_google_fonts: true,
    google_font_variant_threshold: NonZeroU64::new(16),
    max_v8_heap_size_mb: NonZeroU64::new(1024),
    max_v8_execution_time_secs: NonZeroU64::new(10),
    ..Default::default()
})?;
```
::::

::::{interface} server
`vl-convert serve` uses the same startup precedence as the CLI for converter
configuration. Server-only controls such as listeners, budgets, CORS, and
request body limits come from `serve` flags or their `VLC_*` environment
variables.

When an admin listener is enabled, `PUT /admin/config` replaces the live
converter configuration and `PATCH /admin/config` updates selected fields.
Per-request options still apply on top of the active server config. Per-request
Google Fonts and plugin overrides are rejected unless enabled with
`--allow-google-fonts` or `--allow-per-request-plugins`.

```bash
vl-convert serve \
  --vlc-config production.vlc.jsonc \
  --admin-host 127.0.0.1 \
  --admin-port 3001 \
  --admin-api-key "$ADMIN_API_KEY"
```
::::

## JSONC Config Files

`--vlc-config`, Python `load_config()`, and Rust-side config loading use the
same JSONC shape. Comments and trailing commas are accepted.

This example is a hardened application config, not a list of library defaults.
In particular, the library default `base_url` is `true` and the default
`allowed_base_urls` permits HTTP and HTTPS data URLs.

```json
{
  // Worker and data-access defaults.
  "num_workers": 4,
  // default: true; false disables relative data loading
  "base_url": false,
  // default: ["http:", "https:"]; [] blocks data fetches
  "allowed_base_urls": [],

  // Font behavior.
  "auto_google_fonts": false,
  "google_font_variant_threshold": 16,
  "google_fonts": [
    { "family": "Inter" }
  ],
  "embed_local_fonts": false,
  "subset_fonts": true,
  "missing_fonts": "warn",

  // V8 resource controls.
  "max_v8_heap_size_mb": 1024,
  "max_v8_execution_time_secs": 10,
  "gc_after_conversion": false,

  // Plugin controls.
  "vega_plugins": [],
  "plugin_import_domains": [],
  "allow_per_request_plugins": false,
  "max_ephemeral_workers": 2,
  "allow_google_fonts": false,
  "per_request_plugin_import_domains": [],

  // Vega-Lite defaults.
  "default_theme": null,
  "default_format_locale": null,
  "default_time_format_locale": null,
  "themes": {}
}
```

`base_url` accepts `true` for the Vega datasets default, `false` to disable
relative data loading, or a URL/path string. `allowed_base_urls` is an allowlist
of CSP-style patterns; use `[]` in JSONC to block data fetches and `["*"]` only
for trusted inputs.

CLI flags and environment variables use a compact Google Fonts shorthand:
`Inter:400,700italic` is equivalent to a config entry with explicit variants:

```json
{
  "family": "Inter",
  "variants": [
    {"weight": 400, "style": "normal"},
    {"weight": 700, "style": "italic"}
  ]
}
```

The same variant information has surface-specific syntax: Python uses tuple
variants such as `{"family": "Inter", "variants": [(400, "normal")]}`;
JSONC config and the admin API use `{"weight": 400, "style": "normal"}`
objects; CLI flags and environment variables use the compact string form above.

## Environment Variables

Most CLI and server flags have `VLC_*` environment variable equivalents. The
most commonly useful converter variables are:

| Variable | Purpose |
| --- | --- |
| `VLC_CONFIG` | Path to a JSONC config file, or `disabled`. |
| `VLC_BASE_URL` | Relative data base URL: `default`, `disabled`, URL, or path. |
| `VLC_ALLOWED_BASE_URLS` | Semicolon-separated data URL allowlist, or CLI shortcuts `none`, `net`, `all`. |
| `VLC_FONT_DIR` | Font directories. Uses the platform path separator. |
| `VLC_GOOGLE_FONT` | Semicolon-separated Google Font requests such as `Inter:400,700italic`. |
| `VLC_AUTO_GOOGLE_FONTS` | Enable automatic Google Fonts downloads. |
| `VLC_GOOGLE_FONT_VARIANT_THRESHOLD` | Stop admitting additional Google Font families after this many variants. |
| `VLC_GOOGLE_FONTS_CACHE_SIZE_MB` | On-disk Google Fonts cache size. |
| `VLC_MISSING_FONTS` | Missing first-choice font policy: `fallback`, `warn`, or `error`. |
| `VLC_MAX_V8_HEAP_SIZE_MB` | Per-worker V8 heap limit. |
| `VLC_MAX_V8_EXECUTION_TIME_SECS` | V8 execution timeout. |
| `VLC_VEGA_PLUGIN` | Semicolon-separated config-level plugin paths or URLs. |
| `VLC_PLUGIN_IMPORT_DOMAINS` | Semicolon-separated HTTP import domains for config-level plugins. |
| `VLC_LOG_LEVEL`, `VLC_LOG_FORMAT`, `VLC_LOG_FILTER` | Logging defaults. |

Common server-only variables are:

| Variable | Purpose |
| --- | --- |
| `VLC_HOST`, `VLC_PORT`, `VLC_UNIX_SOCKET` | Main listener binding. |
| `VLC_WORKERS` | Persistent converter worker count for `serve`. |
| `VLC_API_KEY` | Bearer token for the main listener. |
| `VLC_ADMIN_HOST`, `VLC_ADMIN_PORT`, `VLC_ADMIN_API_KEY` | Admin listener binding and bearer token. |
| `VLC_MAX_CONCURRENT_REQUESTS`, `VLC_REQUEST_TIMEOUT_SECS`, `VLC_MAX_BODY_SIZE_MB` | Request admission and body limits. |
| `VLC_DRAIN_TIMEOUT_SECS`, `VLC_RECONFIG_DRAIN_TIMEOUT_SECS` | Graceful shutdown and reconfiguration drain windows. |
| `VLC_PER_IP_BUDGET_MS`, `VLC_GLOBAL_BUDGET_MS`, `VLC_GOOGLE_FONT_CACHE_MISS_PENALTY_MS` | Request budget controls. |
| `VLC_CORS_ORIGIN`, `VLC_TRUST_PROXY` | Browser and reverse-proxy controls. |

`PORT` is also honored by common hosting platforms when the server port is
otherwise unset.

`VL_CONVERT_FONT_CACHE_DIR` controls the Google Fonts cache directory at the
Google Fonts crate layer; set it to `none` to disable that on-disk cache.
`VL_CONVERT_BIN` is used by the documentation build tooling to choose a
specific `vl-convert` binary.

Use `--help` on a specific command for the authoritative flag and environment
variable list.
