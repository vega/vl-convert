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

Typed `=BOOL` values on every boolean and a single
`--allowed-base-urls=<value>` flag for URL access control.

```
--vlc-config=<value>          # absolute path to JSONC config file, or `disabled` to skip

--base-url=<value>            # default | disabled | URL with scheme | absolute path
--allowed-base-urls=<value>   # default | none | all | JSON array | @path-to-file

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

`--allowed-base-urls` reserved values: `default` (HTTP/HTTPS, library
default), `none` (block all), `all` (`["*"]`, allow everything incl.
filesystem). Otherwise a JSON array of CSP-style patterns
(`"https:"`, `"https://example.com/"`, `"/data/"`), or `@<path>` to
read the JSON from a file.

To disable all external data access: `--allowed-base-urls=none`. To
allow only specific prefixes:
`--allowed-base-urls='["https://cdn.example.com/"]'`.

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
