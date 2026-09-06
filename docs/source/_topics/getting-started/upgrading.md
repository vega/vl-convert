---
title: Upgrading from 1.x
path: getting-started/upgrading
section: Getting Started
order: 120
interfaces: [python, cli, rust]
---

<!-- topic-body -->

# Upgrading from 1.x

Version 2 keeps the conversions you already use but changes how converters are
configured, tightens data access, and replaces a few options. This page lists
what to change when moving from 1.9 to 2.0, followed by behavior differences to
check even when your code still runs.

::::{interface} python
## Update Your Code

**Options are keyword-only.** Every conversion function takes the
specification positionally and everything else by keyword. A call such as
`vegalite_to_svg(spec, "5.16")` now raises `TypeError`. Write
`vegalite_to_svg(spec, vl_version="5.16")` instead.

**`allowed_base_urls` moved to `configure()`.** The per-call argument was
removed from every conversion function. Set the allowlist once for the
process:

```python
import vl_convert as vlc

vlc.configure(allowed_base_urls=["https://data.example.com/"])
```

**`show_warnings` is deprecated and ignored.** Passing `True` raises a
`DeprecationWarning`. Vega and Vega-Lite warnings now go to the `vl_convert`
logger, so configure Python logging instead. See {doc}`../guides/logging`.

**PDF `scale` was removed.** `vegalite_to_pdf()`, `vega_to_pdf()`, and
`svg_to_pdf()` no longer accept `scale`. PDF output is vector, so change the
chart's `width` and `height` if the page needs a different size.

**New, optional capabilities.** `configure()`, `load_config()`, and
`get_config()` manage process-wide settings such as the worker pool, Google
Fonts, plugins, and JavaScript limits. `vl_convert.asyncio` provides awaitable
conversions. Conversion functions gained `width`, `height`, `background`,
`google_fonts`, and `vega_plugin`, the Vega functions gained `config`, the SVG
output functions for Vega and Vega-Lite input gained `bundle`, and the
scenegraph functions accept `format="msgpack"`. See
{doc}`../advanced/configuration` and
{doc}`../api-reference`.

Python 3.7 and later are still supported, and `vl_version` still accepts both
`"5.16"` and `"v5_16"`.
::::

::::{interface} cli
## Update Your Commands

**Removed and renamed options.** Commands that pass these fail with
`unexpected argument`:

| 1.x | 2.0 |
| --- | --- |
| `--show-warnings` | Removed. Warnings print to standard error by default. Adjust with the global `--log-level`. |
| `-a`, `--allowed-base-url URL`, repeated per URL | Global `--allowed-base-urls` taking a `;`-separated list, or `none`, `net`, or `all`. |
| `-s` on `vl2jpeg` | Removed with `--show-warnings`. |

**The default chart config file is no longer applied.** 1.x used
`~/.config/vl-convert/config.json` as the Vega-Lite config for every command
when that file existed. 2.0 ignores it, and `--config` has no default. To keep
a default chart configuration, put it in a converter config file as a custom
theme and select it with `default_theme`. Save this example at the path printed
by `vl-convert config-path`:

:::{dropdown} vlc-config.jsonc
:open:

```json
{
  "themes": {"house": {"axis": {"labelFont": "Inter", "titleFont": "Inter"}}},
  "default_theme": "house"
}
```
:::

The file loads automatically from that path. You can instead pass the converter
configuration with `--vlc-config`. Alternatively, keep the 1.x chart
configuration as a separate file and pass its path with `--config` on each
chart conversion. See {doc}`../advanced/configuration`.

**The default Vega-Lite version is 6.4.** 1.x commands defaulted to 5.21.
Pass `--vl-version 5.21` to keep the old output for specifications that depend
on Vega-Lite 5 behavior.

**Input and output are optional.** `--input` and `--output` now default to
standard input and standard output, and `vl2url` and `vg2url` gained
`--output`. Commands that pass both options work unchanged. See
{doc}`../advanced/cli-piping`.

**New commands and options.** `vl2fonts`, `vg2fonts`, `vl2sg`, `vg2sg`,
`bundle-js`, `config-path`, and `serve` are new. `--font-dir` is now global and
repeatable, and new global options cover the converter config file, data
access, logging, Google Fonts, plugins, and JavaScript limits. Each has a
`VLC_*` environment variable. Run `vl-convert --help` for the list.
::::

::::{interface} rust
## Update Your Code

Change the dependency to `vl-convert-rs = "2"`, then fix these compile errors.

**Conversion methods return output structs.** Methods that returned `String`
or `Vec<u8>` now return `SvgOutput`, `PngOutput`, `JpegOutput`, `PdfOutput`,
`HtmlOutput`, `ScenegraphOutput`, or `VegaOutput`. Read the result from `svg`,
`data`, `html`, `scenegraph`, or `spec`, and Vega's messages from `logs`.

**Format options are structs.** The positional `scale`, `ppi`, `quality`,
`bundle`, and `renderer` arguments became `PngOpts`, `JpegOpts`, `PdfOpts`,
`SvgOpts`, and `HtmlOpts`. The SVG and PDF methods gained an options argument
too.

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

**`VlOpts` and `VgOpts` changed fields.** `show_warnings` and
`allowed_base_urls` were removed. Put the allowlist in `VlcConfig` and read
warnings from `logs`. Both structs gained `google_fonts`, `vega_plugin`,
`background`, `width`, and `height`, and `VgOpts` gained `config`.

**URL helpers take `UrlOpts`.** `vegalite_to_url(&spec, true)` becomes
`vegalite_to_url(&spec, UrlOpts { fullscreen: true })`, and the same for
`vega_to_url`.

**Converter construction.** `VlConverter::new()` still creates a converter with
default settings. Use `VlConverter::with_config()` to set worker count, data
access, fonts, and limits. Methods take `&self` instead of `&mut self`, and the
worker pool starts on first use. See {doc}`../advanced/rust-converter`.

**Unchanged.** The free functions `converter::svg_to_png()`, `svg_to_jpeg()`,
and `svg_to_pdf()` keep their 1.x signatures, although the converter methods of
the same names are preferred because they apply the data access policy and
Google Fonts settings. `text::register_font_directory()` is unchanged and is
also re-exported at the crate root. The `VlVersion` variants are the same.
::::

## Check Behavior Differences

These differences can change results even when code and commands still run.

**Data and image access is a converter setting with an allowlist.** 1.x
placed no restriction on the URLs a specification could load unless you passed
an allowlist per conversion. 2.0 allows any HTTP or HTTPS URL by default and
blocks local files, for data and images alike. Specifications that load local
data or images need the directory added to `allowed_base_urls`, and relative
data and image URLs resolve against `base_url`. See
{doc}`../guides/data-loading`.

**Warnings are always captured.** 1.x dropped Vega and Vega-Lite warnings
unless you asked for them. 2.0 records them on every conversion and reports
them through each interface's logging, so expect to see messages you did not
see before. They are worth reading: a warning often explains a chart that
renders differently than expected. See {doc}`../guides/logging`.

**PNG pixels differ slightly.** 1.x rasterized the SVG output. 2.0 renders PNG
with Vega's canvas renderer through a built-in Canvas 2D implementation, which
changes anti-aliasing and text rendering at the pixel level. Regenerate image
baselines in tests that compare PNG bytes. JPEG and PDF output still start from
the SVG rendering.

::::{interface} python rust
**Conversions run on a worker pool.** 1.x ran every conversion on one
JavaScript runtime. 2.0 starts a pool on first use, with one worker by default,
and can run conversions concurrently when you configure more. See
{doc}`../advanced/memory-management` for sizing.
::::
