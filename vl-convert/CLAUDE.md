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
