---
title: Upgrading from 1.x
path: getting-started/upgrading
section: Getting Started
order: 120
interfaces: [python, cli, rust]
---

<!-- topic-body -->

# Upgrading from 1.x

Version 2 keeps the conversions you already use but changes some details on how converters are configured, tightens data access, and replaces a few options. This page lists what to change when moving from 1.9 to 2.0, followed by behavior differences to check even when your code still runs.

::::{interface} python
## Update Your Code

**Options are keyword-only.** Every conversion function takes the specification positionally and everything else by keyword. A call such as `vegalite_to_svg(spec, "5.16")` now raises `TypeError`. Write `vegalite_to_svg(spec, vl_version="5.16")` instead.

**`allowed_base_urls` moved to the new `vlc.configure()`.** The per-call argument was removed from every conversion function. Set the allowlist once for the process:

```python
import vl_convert as vlc

vlc.configure(allowed_base_urls=["https://data.example.com/"])
```

**`show_warnings` is deprecated and ignored.** Passing `True` raises a `DeprecationWarning`. Vega and Vega-Lite warnings now go to the `vl_convert` logger, so configure Python logging instead. See {doc}`../guides/logging`.

**PDF `scale` is retained only for backward compatibility.** `vegalite_to_pdf()`, `vega_to_pdf()`, and `svg_to_pdf()` still accept `scale=None` or `scale=1.0`. Other numeric values raise `ValueError`. The parameter never affected PDF output and is deprecated in version 2. Change the chart's `width` and `height` if the page needs a different size.

**New, optional capabilities** include process-wide configuration, Google Fonts, plugins, asynchronous conversions, and more output options. See {doc}`../advanced/configuration` and {doc}`../api-reference`.

Version 2 requires Python 3.10 or later.
::::

::::{interface} cli
## Update Your Commands

**Removed and renamed options.** Commands that pass these fail with `unexpected argument`:

| 1.x | 2.0 |
| --- | --- |
| `--show-warnings` | Removed. Warnings print to standard error by default. Adjust with the global `--log-level`. |
| `-a`, `--allowed-base-url URL`, repeated per URL | Global `--allowed-base-urls` taking a `;`-separated list, or `none`, `net`, or `all`. |
| `-s` on `vl2jpeg` | Removed with `--show-warnings`. |

**The default chart config file is no longer applied.** 1.x used `~/.config/vl-convert/config.json` as the Vega-Lite config for every command when that file existed. 2.0 ignores it, and `--config` has no default. To keep a default chart configuration, put it in a converter config file as a custom theme and select it with `default_theme`. Save this example at the path printed by `vl-convert config-path`:

:::{dropdown} vlc-config.jsonc
:open:

```json
{
  "themes": {"house": {"axis": {"labelFont": "Inter", "titleFont": "Inter"}}},
  "default_theme": "house"
}
```
:::

The file loads automatically from that path. You can instead pass the converter configuration with `--vlc-config`. Alternatively, keep the 1.x chart configuration as a separate file and pass its path with `--config` on each chart conversion. See {doc}`../advanced/configuration`.

**The default Vega-Lite version is 6.4.** 1.x commands defaulted to 5.21. Pass `--vl-version 5.21` to keep the old output for specifications that depend on Vega-Lite 5 behavior.

**Input and output are optional.** `--input` and `--output` now default to standard input and standard output, and `vl2url` and `vg2url` gained `--output`. Commands that pass both options work unchanged. See {doc}`../advanced/cli-piping`.

**New commands and options.** `vl2fonts`, `vg2fonts`, `vl2sg`, `vg2sg`, `bundle-js`, `config-path`, and `serve` are new. `--font-dir` is now global and repeatable, and new global options cover the converter config file, data access, logging, Google Fonts, plugins, and JavaScript limits. Each has a `VLC_*` environment variable. Run `vl-convert --help` for the list.
::::

::::{interface} rust
## Update Your Code

Change the dependency to `vl-convert-rs = "2.0.0-rc6"` to select rc6 or later, then update these API calls.

**Conversion methods return output structs.** Methods that returned `String` or `Vec<u8>` now return `SvgOutput`, `PngOutput`, `JpegOutput`, `PdfOutput`, `HtmlOutput`, `ScenegraphOutput`, or `VegaOutput`. Read the result from `svg`, `data`, `html`, `scenegraph`, or `spec`, and Vega's messages from `logs`.

**Format options are structs.** The positional `scale`, `ppi`, `quality`, `bundle`, and `renderer` arguments became `PngOpts`, `JpegOpts`, `PdfOpts`, `SvgOpts`, and `HtmlOpts`. The SVG and PDF methods gained an options argument too.

```rust
// 1.x
let png: Vec<u8> = converter
    .vegalite_to_png(spec, VlOpts::default(), Some(2.0), None)
    .await?;

// 2.0
let png = converter
    .vegalite_to_png(
        spec,
        VlOpts::default(),
        PngOpts {
            scale: Some(2.0),
            ..Default::default()
        },
    )
    .await?
    .data;
```

**`VlOpts` and `VgOpts` changed fields.** `show_warnings` and `allowed_base_urls` were removed. Put the allowlist in `VlcConfig` and read warnings from `logs`. Both structs gained `google_fonts`, `vega_plugin`, `background`, `width`, and `height`, and `VgOpts` gained `config`.

**URL helpers take `UrlOpts`.** `vegalite_to_url(&spec, true)` becomes `vegalite_to_url(&spec, UrlOpts { fullscreen: true })`, and the same for `vega_to_url`.

**Converter construction.** `VlConverter::new()` still creates a converter with default settings. Use `VlConverter::with_config()` to set worker count, data access, fonts, and limits. Methods take `&self` instead of `&mut self`, and JavaScript workers start when needed. See {doc}`../advanced/rust-converter`.

**SVG conversions use converter methods.** The free `converter::svg_to_png()`, `svg_to_jpeg()`, and `svg_to_pdf()` functions were removed. Use the async methods on `VlConverter` with `PngOpts`, `JpegOpts`, or `PdfOpts`. They apply the converter's font and image-access settings without starting JavaScript workers. See {doc}`../guides/svg-conversions`.

**Public imports.** Use the crate-root `register_font_directory()` function. Implementation modules such as `text`, `html`, `extract`, and `module_loader` are now private. Import supported types, version constants, and locale maps from `vl_convert_rs` instead. Use `VlcConfig` in place of the removed `VlConverterConfig` alias.
::::

## Check Behavior Differences

These differences can change results even when code and commands still run.

**The allowlist now covers images as well as data.** HTTP and HTTPS URLs remain allowed by default. Custom `allowed_base_urls` settings now restrict image loading too, so include any image locations your charts use. Local data and image files remain blocked unless the allowlist permits them.

**The default base URL has changed.** The default base_url, that relative URLs resolve against, is now `https://cdn.jsdelivr.net/npm/vega-datasets@v3.2.1/` instead of `https://vega.github.io/vega-datasets/`. If your allowlist only permits the old host, update it or set `base_url` to the old location. See {doc}`../guides/data-loading`.

**Absolute data and image paths refer to local files.** Paths such as `/data/cars.csv` and, on Windows, `C:\data\cars.csv` no longer resolve against `base_url`. Allow the directory to load them without a `file://` prefix. For a remote resource, remove the leading slash to resolve `data/cars.csv` against `base_url`, or use an explicit HTTP or HTTPS URL. Chart hyperlinks keep their existing URL behavior.

**Warnings are always captured.** 1.x dropped Vega and Vega-Lite warnings unless you asked for them. 2.0 records them on every conversion and reports them through each interface's logging, so expect to see messages you did not see before. They are worth reading! A warning often explains why a chart renders differently than expected. See {doc}`../guides/logging`.

**PNG pixels may differ slightly.** To build a PNG image, 1.x first generated an SVG and then rasterized the SVG using the `resvg` crate. This approach generally worked well, but one limitation was that it did not support Vega's [label transform](https://vega.github.io/vega/docs/transforms/label/). Also, the SVG generation followed by SVG parsing had some overhead for charts with many mark instances.

Version 2.0 renders PNGs using Vega's canvas renderer with a new Canvas 2D implementation based on [tinyskia](https://github.com/linebender/tiny-skia) and [cosmic-text](https://github.com/pop-os/cosmic-text). This may result in small pixel-level changes to PNGs. Regenerate image baselines in tests that compare PNG bytes.

::::{interface} python rust
**Conversions run on a worker pool.** 1.x ran every conversion on one JavaScript runtime. 2.0 starts a pool on first use, with one worker by default, and can run conversions concurrently when you configure multiple workers. See {doc}`../advanced/memory-management` for sizing.
::::
