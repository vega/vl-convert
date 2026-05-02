# vl-convert (CLI)

Command-line interface wrapping vl-convert-rs. Built with Clap.

## Subcommands

**Vega-Lite**: vl2vg, vl2svg, vl2png, vl2jpeg, vl2pdf, vl2url, vl2html
**Vega**: vg2svg, vg2png, vg2jpeg, vg2pdf, vg2url, vg2html
**SVG**: svg2png, svg2jpeg, svg2pdf
**Utility**: ls-themes, cat-theme, config-path
**Server**: serve (HTTP server backed by `vl-convert-server`; see
`vl-convert-server/CLAUDE.md` for protocol invariants and the
`Notes for downstream binary authors` section. The lifecycle wiring
— SIGTERM, ready-JSON, drain watchdog, stdin-EOF watcher — lives in
`vl-convert/src/serve.rs`.)

## Common Arguments

```
-i, --input       Input file (required)
-o, --output      Output file (required)
-v, --vl-version  Vega-Lite version (default: 5.21)
-t, --theme       Theme name
-c, --config      Config JSON path
--scale           Scale factor (default: 1.0)
--ppi             Pixels per inch (default: 72.0)
```

## Global config flags (typed values)

Typed `=BOOL` values on every boolean and a single
`--allowed-base-urls=<value>` flag for URL access control.

```
--vlc-config=<value>          # absolute path to JSONC config file, or `disabled` to skip

--base-url=<value>            # default | disabled | URL with scheme | absolute path
--allowed-base-urls=<value>   # none | net | all | JSON array | @path-to-file

--auto-google-fonts=BOOL      # default: false
--embed-local-fonts=BOOL      # default: false
--subset-fonts=BOOL           # default: true
--gc-after-conversion=BOOL    # default: false
--missing-fonts <fallback|warn|error>
--font-dir <PATH>             # repeatable: pass multiple times for multiple dirs
--google-font <Family[:variants]>
--google-fonts-cache-size-mb <MB>   # 0 = library default
--default-theme <THEME|null>            # null clears the value
--default-format-locale <LOCALE|JSON|@FILE|null>
--default-time-format-locale <LOCALE|JSON|@FILE|null>
--themes <JSON|@FILE|null>    # custom named themes map
--max-v8-heap-size-mb <n>     # 0 = no cap
--max-v8-execution-time-secs <n>   # 0 = no cap
--vega-plugin <path|url|inline-esm>
--plugin-import-domains <csv>
--log-level <error|warn|info|debug>
--log-format <text|json>      # default: text
--log-filter <DIRECTIVE>      # raw EnvFilter directive; wins over --log-level
```

All globals accept clap's `global = true` placement, so they may be
passed before *or* after the subcommand keyword. `--font-dir` calls
`vl_convert_rs::set_font_directories` once at startup with replace
semantics — the entire list is authoritative.

`--default-theme`, `--default-format-locale`,
`--default-time-format-locale`, and `--themes` accept the literal string
`null` (any case) to clear a value loaded from `--vlc-config`. The
distinction "flag not passed" vs "flag passed with `null`" is preserved
by parsing the `null` literal at consumption time in `io_utils.rs`
(see `parse_nullable_string_arg`, `parse_themes_json`, and the
`null`-literal branches in `parse_format_locale_option` /
`parse_time_format_locale_option`). The CLI fields stay flat
`Option<String>` because clap 4's custom value parsers panic at
runtime on `Option<Option<T>>` field types.

## Logging stack

Logging routes through `tracing-subscriber` (configured by
`vl_convert_server::init_tracing`) for every subcommand. There is no
`env_logger` dependency. All log lines go to stderr; stdout is
reserved for `serve --ready-json` (single JSON line) and conversion
output. Output format differs from the v1.x `env_logger` line shape
(`[INFO  vl_convert] msg` → `<timestamp> INFO vl_convert: msg`); see
the v2.x changelog.

`=BOOL` accepts `true|false|1|0|yes|no|on|off` (case-insensitive). Bare
boolean flags (e.g. `--auto-google-fonts`) resolve to `true`.

`--allowed-base-urls` reserved values: `none` (block all), `net`
(HTTP/HTTPS only, no filesystem), `all` (`["*"]`, allow everything
incl. filesystem). Otherwise a JSON array of CSP-style patterns
(`"https:"`, `"https://example.com/"`, `"/data/"`), or `@<path>` to
read the JSON from a file.

To disable all external data access: `--allowed-base-urls=none`. To
allow only specific prefixes:
`--allowed-base-urls='["https://cdn.example.com/"]'`.

## Testing

```bash
pixi run test-cli
```

Tests live in `tests/test_formats.rs` (image-comparison conversions),
`tests/test_logging.rs` (log-level filtering), `tests/test_stdin_stdout.rs`
(streaming I/O), and `tests/test_serve_subprocess.rs` (binary lifecycle:
ready-JSON, SIGTERM cleanup, stdin-EOF watcher, bind-failure ordering;
unix-only). Common helpers are in `tests/common/mod.rs`; UDS HTTP
helpers (raw hyper + `tokio::net::UnixStream`) are duplicated from
`vl-convert-server/tests/common/mod.rs` into `tests/common/uds.rs` to
avoid graduating them into the published `vl-convert-server` API.

Test crates: `assert_cmd` (subprocess spawning), `rstest`
(parameterized), `dssim` (image comparison, threshold 0.0001),
`hyper`/`hyper-util`/`http-body-util`/`bytes` (UDS HTTP for
subprocess tests), `tempfile` (per-test sockets/configs).

## Key Patterns

- Error handling: `anyhow::Error` with `bail!()`
- Config fallback: `~/.config/vl-convert/config.json`
- Version parsing: Accepts "5.21" or "v5_21"
