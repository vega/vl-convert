---
title: Configuration
path: advanced/configuration
section: Advanced
order: 400
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Configuration

The converter configuration holds settings that apply across all conversions: data access policy, fonts, plugins, worker resources, and default themes and locales. Values such as output format, dimensions, and scale belong to the individual conversion instead. See {doc}`conversion-overrides` for those.

## JSONC Configuration Files

Python, Rust, the CLI, and the server all read the same configuration format. This example limits worker resources and blocks external data and image loads. Save it as `production.vlc.jsonc`:

:::{dropdown} production.vlc.jsonc
:open:

```{literalinclude} /_examples/production.vlc.jsonc
:language: json
```
:::

Note that JSONC is JSON with support for comments and trailing commas.

Fields omitted from the file use their built-in defaults. This example changes some of those defaults: `base_url` normally points at the Vega datasets CDN, and `allowed_base_urls` permits any HTTP or HTTPS URL. Setting `base_url` to `false` rejects relative data and image URLs. Setting `allowed_base_urls` to an empty list rejects every external URL and local file.

`allowed_base_urls` accepts Content Security Policy-style patterns. Grant only the paths, URL prefixes, or schemes the application needs, and use `"*"` only when every input is trusted. See {doc}`../guides/security`.

::::{interface} python
## Configure a Python Process

The Python package keeps one shared converter per process. Configure it during application startup, before concurrent conversions begin.

Load the file with `load_config()`, then override selected fields with `configure()`. This example changes `num_workers` from 4 to 2 and prints the active settings with `get_config()`:

```{literalinclude} /_examples/inspect-config.py
:language: python
```

Output:

```{program-output} python inspect-config.py
:cwd: /_examples
:language: json
```

`load_config()` replaces every setting with the built-in defaults plus the file's contents. `configure()` changes only the fields you pass and leaves the rest unchanged. Passing `None` for a field resets it to the built-in default. These settings apply to later conversions in the current process.

You can also call `configure()` without loading a file.

Without a path, `load_config()` reads the platform-standard file returned by `get_config_path()`. If that file does not exist, the converter resets to the built-in defaults.

### Warm Workers Before Serving Traffic

Workers normally start on the first conversion. After configuring the process, call `warm_up_workers()` during application startup when first-request latency matters:

```python
vlc.warm_up_workers()
```

`get_worker_memory_usage()` also starts the pool if it is not running yet and reports each worker's JavaScript heap statistics. See {doc}`memory-management` for memory and execution limits.

### Change Configuration Safely

Changing a setting with `configure()` creates a replacement converter with a fresh worker pool. Passing unchanged values keeps the current pool. `load_config()` always creates a replacement, even when the settings are unchanged.

Conversions already in progress finish on the old pool. New calls use the replacement, whose workers start lazily. Frequent reconfiguration discards warm workers, so use per-call options for scale, dimensions, theme, and locale. Reserve `configure()` for process-level policy. See {doc}`conversion-overrides`.

`vl_convert.asyncio` shares the same converter and configuration. Configure the process before starting concurrent work through either API. See {doc}`python-async` for async usage.
::::

::::{interface} cli
## Configure a CLI Command

Pass the file with `--vlc-config` and use flags to override individual settings. This example lowers the heap limit from 1024 to 512 MB and uses `chart.vl.json` from {doc}`../getting-started/quick-start`:

```console
$ vl-convert --vlc-config production.vlc.jsonc \
>   --max-v8-heap-size-mb 512 \
>   vl2png --input chart.vl.json --output chart.png
```

The CLI takes each setting from the first source that provides it:

```text
command-line flags
VLC_* environment variables
--vlc-config or the platform-default config file
built-in defaults
```

Relative configuration paths resolve from the current working directory. When `--vlc-config` is omitted, the CLI loads the platform-default file if it exists. Print that path with `vl-convert config-path`, or pass `--vlc-config disabled` to skip config files entirely.

Global options can appear before or after the conversion command. Run `vl-convert --help` for the global options and `vl-convert vl2png --help` for a command's own options.
::::

::::{interface} rust
## Configure a Rust Converter

Load the file with `VlcConfig::from_file()`, update its fields, then pass it to `VlConverter::with_config()`. This example changes the worker count from 4 to 2:

```rust
use std::num::NonZeroU64;
use std::path::Path;
use vl_convert_rs::{VlcConfig, VlConverter};

let mut config = VlcConfig::from_file(Path::new("production.vlc.jsonc"))?;
config.num_workers = NonZeroU64::new(2).unwrap();
let converter = VlConverter::with_config(config)?;
```

To configure without a file, start with `VlcConfig::default()`. The converter keeps its settings for its lifetime. See {doc}`rust-converter` for sharing and reusing it.
::::

::::{interface} server
## Configure a Server

Pass the file with `--vlc-config` and use flags to override individual settings. This example lowers the heap limit from 1024 to 512 MB and starts the server on port 3000:

```console
$ vl-convert serve \
>   --vlc-config production.vlc.jsonc \
>   --max-v8-heap-size-mb 512 \
>   --port 3000
```

Converter settings use the same precedence as the other CLI commands:

```text
command-line flags
VLC_* environment variables
--vlc-config or the platform-default config file
built-in defaults
```

Converter options are global and can appear before or after `serve`. Server-specific options, such as addresses, ports, authentication, request limits, and budgets, follow `serve`.

When the admin API is enabled, `PUT /admin/config` replaces the live converter configuration and `PATCH /admin/config` changes selected fields. New requests use the updated configuration. See {doc}`/server/admin-api`.

Per-request values take priority over the server configuration, but requests cannot select Google Fonts or supply plugin code unless the server enables those capabilities explicitly.
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
| `VLC_HOST`, `VLC_PORT`, `VLC_UNIX_SOCKET` | Server API address, port, or Unix socket |
| `VLC_WORKERS` | Persistent conversion worker count |
| `VLC_API_KEY` | Bearer token for the server API |
| `VLC_ADMIN_HOST`, `VLC_ADMIN_PORT` | Admin API address and port |
| `VLC_ADMIN_API_KEY` | Bearer token for the admin API |
| `VLC_MAX_CONCURRENT_REQUESTS` | Maximum requests admitted at once |
| `VLC_REQUEST_TIMEOUT_SECS` | Per-request timeout |
| `VLC_MAX_BODY_SIZE_MB` | Maximum request body size |
| `VLC_DRAIN_TIMEOUT_SECS` | Graceful shutdown window |
| `VLC_PER_IP_BUDGET_MS`, `VLC_GLOBAL_BUDGET_MS` | Render-time budgets |
| `VLC_CORS_ORIGIN` | Allowed browser origins |
| `VLC_TRUST_PROXY` | Whether to use proxy-supplied client addresses |

`PORT` is also accepted when `VLC_PORT` and `--port` are unset. Run `vl-convert serve --help` for the complete list.
::::
