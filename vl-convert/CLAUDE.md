# vl-convert CLI

Command-line wrapper around `vl-convert-rs`, built with clap. Public
option inventory belongs in `vl-convert --help` and `README.md`; keep
this file focused on implementation rules that are easy for agents to
break.

## Boundaries

- `serve` is the reference binary for `vl-convert-server`. Lifecycle
  wiring lives in `src/serve.rs`: signal handling, ready JSON, drain
  watchdog, stdin EOF parent watcher, and OpenAPI dumping.
- Protocol and server-library invariants live in
  `../vl-convert-server/CLAUDE.md`. Public embedding guidance lives in
  `../vl-convert-server/README.md`.
- Do not duplicate full flag lists here. They drift quickly; reference
  clap help and the README instead.

## Config and Env Fallback

Configuration-bearing globals and serve-local runtime flags use clap
`env` fallback. Effective precedence is:

```text
CLI flag > VLC_* env var > --vlc-config > library default
```

Special cases:

- `--port` resolves as `--port > VLC_PORT > PORT > 3000`.
- `--vlc-config` uses `VLC_CONFIG`, not `VLC_VLC_CONFIG`.
- Boolean options accept typed values such as `true`, `false`, `1`, `0`,
  `yes`, `no`, `on`, and `off`; bare boolean flags resolve to `true`.
- All globals use clap `global = true`, so they may appear before or
  after the subcommand keyword.

Vec-shaped env/list values split on `;`:

- `--allowed-base-urls`
- `--google-font`
- `--vega-plugin`
- `--plugin-import-domains`
- `--per-request-plugin-import-domains`

The only delimiter exception is `--font-dir`, which uses the OS PATH
separator (`:` on Unix, `;` on Windows) and calls
`vl_convert_rs::set_font_directories` once at startup with replace
semantics.

## Parser Gotchas

`--allowed-base-urls` reserved literals (`none`, `net`, `all`) expand
only when they are the sole value. Mixed values such as
`none;https://example.com/` are treated as literal CSP patterns.

`--vega-plugin` accepts file paths and URLs only. Inline ESM belongs in
JSONC config, the Rust/Python APIs, or HTTP request bodies, where it is
not ambiguous with CLI list delimiters.

`--default-theme`, `--default-format-locale`,
`--default-time-format-locale`, and `--themes` accept the literal string
`null` to clear config-loaded values. Keep these CLI fields as flat
`Option<String>` and parse the `null` literal at consumption time in
`io_utils.rs`; clap 4 custom value parsers panic at runtime on
`Option<Option<T>>` field types.

## Stdout and Logging

All tracing output must go to stderr. Stdout is reserved for conversion
output and for the single ready JSON line emitted by
`vl-convert serve --ready-json`.

Logging is initialized through `vl_convert_server::init_tracing`; do
not reintroduce `env_logger` in this crate. `--log-filter` is the raw
`tracing_subscriber::EnvFilter` escape hatch and takes precedence over
`--log-level`.

## Tests

Run CLI tests with:

```bash
pixi run test-cli
```

Relevant coverage:

- `tests/test_formats.rs`: image-comparison conversions.
- `tests/test_logging.rs`: log filtering and stderr/stdout behavior.
- `tests/test_stdin_stdout.rs`: streaming I/O.
- `tests/test_serve_subprocess.rs`: binary lifecycle, ready JSON,
  SIGTERM cleanup, stdin EOF watcher, and bind-failure ordering.
- `tests/test_env_vars.rs` and `src/cli_types.rs::tests`: env fallback,
  delimiter splitting, reserved literals, and parser edge cases.

UDS HTTP helpers are duplicated into `tests/common/uds.rs` from the
server test harness to avoid adding transport helpers to the published
`vl-convert-server` API.
