# vl-convert (CLI)

Command-line interface wrapping vl-convert-rs. Built with Clap.

## Subcommands

**Vega-Lite**: vl2vg, vl2svg, vl2png, vl2jpeg, vl2pdf, vl2url, vl2html
**Vega**: vg2svg, vg2png, vg2jpeg, vg2pdf, vg2url, vg2html
**SVG**: svg2png, svg2jpeg, svg2pdf
**Utility**: ls-themes, cat-theme

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

The CLI mirrors `vl-convert-server`'s flag idiom: typed `=BOOL` values
on every boolean, a `--data-access=MODE` enum for URL access control,
and `--allowed-base-urls=JSON|@FILE` for the explicit allowlist.

```
--vlc-config <path>           # JSONC converter config file path
--load-config=BOOL            # whether to load the config file (default: true)

--base-url=<value>            # default | disabled | <URL/path>
--data-access=<mode>          # default | none | all | allowlist
--allowed-base-urls=<JSON|@FILE>
                              # JSON array literal or @path-to-file with one;
                              # auto-infers --data-access=allowlist if mode is omitted

--auto-google-fonts=BOOL      # default: false
--embed-local-fonts=BOOL      # default: false
--subset-fonts=BOOL           # default: true
--gc-after-conversion=BOOL    # default: false
--missing-fonts <fallback|warn|error>
--font-dir <dir>
--google-font <Family[:variants]>
--max-v8-heap-size-mb <n>     # 0 = no cap
--max-v8-execution-time-secs <n>   # 0 = no cap
--vega-plugin <path|url|inline-esm>
--plugin-import-domains <csv>
--log-level <error|warn|info|debug>
```

`=BOOL` accepts `true|false|1|0|yes|no|on|off` (case-insensitive). Bare
boolean flags (e.g. `--auto-google-fonts`) resolve to `true`.

To disable all external data access:
`--data-access=none`. To allow only specific prefixes:
`--data-access=allowlist --allowed-base-urls='["https://cdn.example.com/"]'`.

## Testing

```bash
pixi run test-cli
```

Tests in `tests/test_cli.rs` use:
- `assert_cmd` for command execution
- `rstest` for parameterized tests
- `dssim` for image comparison (threshold: 0.0001)

## Key Patterns

- Error handling: `anyhow::Error` with `bail!()`
- Config fallback: `~/.config/vl-convert/config.json`
- Version parsing: Accepts "5.21" or "v5_21"
