---
title: Configuration
path: advanced/configuration
section: Advanced
order: 405
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Configuration

Converter configuration sets defaults for operations that need shared or
persistent state. It controls data access, fonts, plugins, worker resources,
and default Vega-Lite themes and locales. Options such as output format,
dimensions, and scale normally belong to an individual conversion.

Only set the fields your application needs. See {doc}`conversion-overrides`
for the options that can change per conversion.

::::{interface} python
## Configure a Python Process

`configure()` changes the supplied fields and leaves the other active settings
unchanged. The settings apply to later conversions in the current Python
process.

```python
import vl_convert as vlc

vlc.configure(
    allowed_base_urls=[],
    auto_google_fonts=True,
    max_v8_heap_size_mb=1024,
    max_v8_execution_time_secs=10,
)
```

`get_config()` returns the active settings. `load_config()` replaces all active
settings with values from a JSONC file and built-in defaults. Call
`configure()` after `load_config()` when code must override selected file
settings.

```python
vlc.load_config("production.vlc.jsonc")
vlc.configure(num_workers=4)
print(vlc.get_config())
```

When no path is passed, `load_config()` uses the platform-standard path
returned by `get_config_path()`. A missing file at that standard path resets
the converter to built-in defaults.
::::

::::{interface} cli
## Configure a CLI Command

The CLI resolves settings in this order, from highest to lowest priority:

```text
command-line flags
VLC_* environment variables
--vlc-config or the platform-default config file
built-in defaults
```

Pass an absolute JSONC file path with `--vlc-config`. If the option is omitted,
the CLI loads the platform-default file when it exists. Use the reserved value
`disabled` to skip config-file loading.

```bash
VLC_AUTO_GOOGLE_FONTS=true \
vl-convert --vlc-config disabled \
  --google-font-variant-threshold 16 \
  vl2png --input chart.vl.json --output chart.png
```

Global options must appear before the conversion subcommand. Run
`vl-convert --help` for global options and `vl-convert vl2png --help` for
command-specific options.
::::

::::{interface} rust
## Configure a Rust Converter

Construct `VlcConfig` and pass it to `VlConverter::with_config()`. The converter
keeps those settings for its lifetime.

```rust
use std::num::NonZeroU64;
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    allowed_base_urls: Vec::new(),
    auto_google_fonts: true,
    max_v8_heap_size_mb: NonZeroU64::new(1024),
    max_v8_execution_time_secs: NonZeroU64::new(10),
    ..Default::default()
})?;
```

Use `VlcConfig::from_file()` when your application needs the shared JSONC file
format. Environment-variable and file precedence are application concerns in
the Rust library. The library does not apply them automatically.
::::

::::{interface} server
## Configure a Server

Converter settings follow the same startup precedence as other CLI commands:

```text
command-line flags
VLC_* environment variables
--vlc-config or the platform-default config file
built-in defaults
```

Options after `serve` configure HTTP behavior, including listeners,
authentication, request limits, and budgets. Global converter options appear
before `serve`.

```bash
vl-convert --vlc-config production.vlc.jsonc \
  serve --host 127.0.0.1 --port 3000
```

An enabled admin listener can replace the live converter configuration with
`PUT /admin/config` or update selected fields with `PATCH /admin/config`. New
requests use the updated configuration. See {doc}`/server/admin-api` for the
request schema and authentication requirements.

Per-request values take priority over the active server configuration.
Requests cannot select Google Fonts or supply plugin code unless the server
enables those capabilities explicitly.
::::

## JSONC Configuration Files

JSONC is JSON with comments and trailing commas. The same field names are used
by Python `load_config()`, Rust `VlcConfig::from_file()`, the CLI, and the
server. This example favors predictable resource use and blocks external data
fetches:

```json
{
  // Persistent conversion workers
  "num_workers": 4,

  // Data loading
  "base_url": false,
  "allowed_base_urls": [],

  // Fonts
  "auto_google_fonts": false,
  "google_fonts": [
    {"family": "Inter", "variants": [{"weight": 400, "style": "normal"}]}
  ],
  "missing_fonts": "warn",

  // JavaScript resource limits
  "max_v8_heap_size_mb": 1024,
  "max_v8_execution_time_secs": 10,

  // Vega extensions
  "vega_plugins": [],
  "plugin_import_domains": [],

  // Vega-Lite defaults
  "default_theme": null,
  "default_format_locale": null,
  "default_time_format_locale": null
}
```

This is an example, not a copy of the built-in defaults. By default,
`base_url` uses the Vega datasets base URL and `allowed_base_urls` permits HTTP
and HTTPS data URLs. Set `base_url` to `false` to reject relative data URLs.
Set `allowed_base_urls` to an empty list to reject all absolute data URLs.

`allowed_base_urls` accepts Content Security Policy-style patterns. Grant only
the paths, URL prefixes, or schemes that the application needs. Use `"*"` only
when every input is trusted. See {doc}`../guides/security` for examples.

::::{interface} python
Python dictionaries passed directly to `configure()` represent font variants
as `(weight, style)` tuples. JSONC files use objects with `weight` and `style`
fields, as shown above.
::::

::::{interface} cli
## Common Environment Variables

Most global flags have `VLC_*` equivalents. Repeatable values use semicolons as
separators, except font directories, which use the platform path separator.

| Variable | Purpose |
| --- | --- |
| `VLC_CONFIG` | Absolute JSONC config path or `disabled` |
| `VLC_BASE_URL` | Relative-data base URL, path, `default`, or `disabled` |
| `VLC_ALLOWED_BASE_URLS` | Data URL allowlist or the shortcut `none`, `net`, or `all` |
| `VLC_FONT_DIR` | Additional font directories |
| `VLC_GOOGLE_FONT` | Google Font requests such as `Inter:400,700italic` |
| `VLC_AUTO_GOOGLE_FONTS` | Whether to download missing first-choice fonts from Google Fonts |
| `VLC_MISSING_FONTS` | `fallback`, `warn`, or `error` |
| `VLC_MAX_V8_HEAP_SIZE_MB` | Heap limit for each JavaScript worker |
| `VLC_MAX_V8_EXECUTION_TIME_SECS` | JavaScript execution timeout |
| `VLC_VEGA_PLUGIN` | Plugin file paths or URLs |
| `VLC_PLUGIN_IMPORT_DOMAINS` | Allowed HTTP import domains for startup plugins |
| `VLC_LOG_LEVEL`, `VLC_LOG_FORMAT`, `VLC_LOG_FILTER` | Logging settings |

Run `vl-convert --help` for the complete and authoritative list.
::::

::::{interface} server
## Common Environment Variables

Most global and `serve` flags have `VLC_*` equivalents. This table lists the
server-specific variables used most often:

| Variable | Purpose |
| --- | --- |
| `VLC_HOST`, `VLC_PORT`, `VLC_UNIX_SOCKET` | Main listener binding |
| `VLC_WORKERS` | Persistent conversion worker count |
| `VLC_API_KEY` | Bearer token for the main listener |
| `VLC_ADMIN_HOST`, `VLC_ADMIN_PORT` | Admin listener binding |
| `VLC_ADMIN_API_KEY` | Bearer token for the admin listener |
| `VLC_MAX_CONCURRENT_REQUESTS` | Maximum requests admitted at once |
| `VLC_REQUEST_TIMEOUT_SECS` | Per-request timeout |
| `VLC_MAX_BODY_SIZE_MB` | Maximum request body size |
| `VLC_DRAIN_TIMEOUT_SECS` | Graceful shutdown window |
| `VLC_PER_IP_BUDGET_MS`, `VLC_GLOBAL_BUDGET_MS` | Render-time budgets |
| `VLC_CORS_ORIGIN` | Allowed browser origins |
| `VLC_TRUST_PROXY` | Whether to use proxy-supplied client addresses |

`PORT` is also accepted when `VLC_PORT` and `--port` are unset. Run
`vl-convert serve --help` for the complete and authoritative list.
::::
