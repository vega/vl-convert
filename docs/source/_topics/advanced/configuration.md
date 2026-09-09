---
title: Configuration
path: advanced/configuration
section: Advanced
order: 400
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Configuration

Converter configuration holds settings that outlive a single conversion: data access policy, fonts, plugins, worker resources, and default themes and locales. Values such as output format, dimensions, and scale belong to the individual conversion instead. See {doc}`conversion-overrides` for those.

Set only the fields your application needs. Every other field keeps its built-in default.

::::{interface} python
## Configure a Python Process

`configure()` changes the fields you pass and leaves the rest unchanged. Passing `None` for a field resets it to the built-in default. The settings apply to later conversions in the current process.

```python
import vl_convert as vlc

vlc.configure(
    allowed_base_urls=[],
    auto_google_fonts=True,
    max_v8_heap_size_mb=1024,
    max_v8_execution_time_secs=10,
)
```

`get_config()` returns the active settings. `load_config()` replaces every setting with the built-in defaults plus the contents of a JSONC file. Call `configure()` after `load_config()` when code must override the file:

```python
vlc.load_config("production.vlc.jsonc")
vlc.configure(num_workers=4)
print(vlc.get_config())
```

Without a path, `load_config()` reads the platform-standard file returned by `get_config_path()`. If that file does not exist, the converter resets to the built-in defaults. See {doc}`python-configuration` for how reconfiguration affects running workers.
::::

::::{interface} cli
## Configure a CLI Command

The CLI takes each setting from the first source that provides it:

```text
command-line flags
VLC_* environment variables
--vlc-config or the platform-default config file
built-in defaults
```

`--vlc-config` takes a path to a JSONC file. A relative path resolves from the current working directory. When the option is omitted, the CLI loads the platform-default file if it exists. Print that path with `vl-convert config-path`, or pass `--vlc-config disabled` to skip config files entirely. This configuration excerpt uses `chart.vl.json` from {doc}`../getting-started/quick-start` to make the precedence example concrete.

```console
$ VLC_AUTO_GOOGLE_FONTS=true \
> vl-convert --vlc-config disabled \
>   --google-font-variant-threshold 16 \
>   vl2png --input chart.vl.json --output chart.png
```

Global options go before the conversion command. Run `vl-convert --help` for the global options and `vl-convert vl2png --help` for a command's own options.
::::

::::{interface} rust
## Configure a Rust Converter

Build a `VlcConfig` and pass it to `VlConverter::with_config()`. The converter keeps those settings for its lifetime.

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

`VlcConfig::from_file()` reads the shared JSONC format. The library does not read environment variables or apply precedence rules. Those are the application's responsibility. See {doc}`rust-converter` for sharing and reusing the converter.
::::

::::{interface} server
## Configure a Server

Converter settings use the same precedence as the other CLI commands:

```text
command-line flags
VLC_* environment variables
--vlc-config or the platform-default config file
built-in defaults
```

Options after `serve` configure HTTP behavior: listeners, authentication, request limits, and budgets. Converter options go before `serve`.

```console
$ vl-convert serve \
>   --vlc-config production.vlc.jsonc \
>   --port 3000
```

When the admin listener is enabled, `PUT /admin/config` replaces the live converter configuration and `PATCH /admin/config` changes selected fields. New requests use the updated configuration. See {doc}`/server/admin-api`.

Per-request values take priority over the server configuration, but requests cannot select Google Fonts or supply plugin code unless the server enables those capabilities explicitly.
::::

## JSONC Configuration Files

JSONC is JSON with comments and trailing commas. Python `load_config()`, Rust `VlcConfig::from_file()`, the CLI, and the server all read the same field names. This example favors predictable resource use and blocks external data fetches. Save it as `production.vlc.jsonc` to use it with the commands above:

:::{dropdown} production.vlc.jsonc
:open:

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
:::

This is an example, not the built-in defaults. By default, `base_url` points at the Vega datasets CDN and `allowed_base_urls` permits any HTTP or HTTPS URL. Set `base_url` to `false` to reject relative data and image URLs, and set `allowed_base_urls` to an empty list to reject every external URL and local file. Misspelled field names are ignored rather than rejected.

`allowed_base_urls` accepts Content Security Policy-style patterns. Grant only the paths, URL prefixes, or schemes the application needs, and use `"*"` only when every input is trusted. See {doc}`../guides/security`.

::::{interface} python
Python dictionaries passed to `configure()` represent font variants as `(weight, style)` tuples. JSONC files use objects with `weight` and `style` fields, as shown above.
::::

::::{interface} cli
## Common Environment Variables

Most global flags have `VLC_*` equivalents. Repeatable values are separated by semicolons, except font directories, which use the platform path separator.

| Variable | Purpose |
| --- | --- |
| `VLC_CONFIG` | JSONC config path or `disabled`. Relative paths resolve from the working directory |
| `VLC_BASE_URL` | Base for relative data and image URLs: a URL, a path, `default`, or `disabled` |
| `VLC_ALLOWED_BASE_URLS` | Data and image URL allowlist, or the shortcut `none`, `net`, or `all` |
| `VLC_FONT_DIR` | Additional font directories |
| `VLC_GOOGLE_FONT` | Google Font requests such as `Inter:400,700italic` |
| `VLC_AUTO_GOOGLE_FONTS` | Whether to download missing first-choice fonts from Google Fonts |
| `VLC_MISSING_FONTS` | `fallback`, `warn`, or `error` |
| `VLC_MAX_V8_HEAP_SIZE_MB` | Heap limit for each JavaScript worker |
| `VLC_MAX_V8_EXECUTION_TIME_SECS` | JavaScript execution timeout |
| `VLC_VEGA_PLUGIN` | Plugin file paths or URLs |
| `VLC_PLUGIN_IMPORT_DOMAINS` | Allowed HTTP import domains for startup plugins |
| `VLC_LOG_LEVEL`, `VLC_LOG_FORMAT`, `VLC_LOG_FILTER` | Logging settings |

Run `vl-convert --help` for the complete list.
::::

::::{interface} server
## Common Environment Variables

Most global and `serve` flags have `VLC_*` equivalents. These are the server-specific variables used most often:

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

`PORT` is also accepted when `VLC_PORT` and `--port` are unset. Run `vl-convert serve --help` for the complete list.
::::
