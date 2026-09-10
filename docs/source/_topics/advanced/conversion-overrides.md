---
title: Conversion Overrides
path: advanced/conversion-overrides
section: Advanced
order: 405
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Conversion Overrides

An override changes one conversion without touching the converter configuration. Use {doc}`configuration` instead when the same value should apply to every conversion.

| Override | Applies to | Effect |
| --- | --- | --- |
| `theme` | Vega-Lite | Applies a built-in or custom theme |
| `config` | Vega-Lite and Vega | Merges a Vega or Vega-Lite configuration object into the input |
| `format_locale` | Vega-Lite and Vega | Selects number formatting rules |
| `time_format_locale` | Vega-Lite and Vega | Selects date and time formatting rules |
| `background`, `width`, `height` | Vega-Lite and Vega | Sets the chart background and logical size before rendering |
| `scale` | PNG and JPEG | Multiplies raster pixel dimensions |
| `ppi` | PNG | Combines with `scale` as `scale * ppi / 72` |
| `quality` | JPEG | Sets encoding quality from `0` through `100`, default `90` |
| `bundle` | SVG and HTML | Embeds fonts and images in SVG, or browser dependencies in HTML |
| `renderer` | HTML | Selects `svg`, `canvas`, or `hybrid` rendering in the browser |
| `vl_version` | Vega-Lite | Selects a bundled Vega-Lite compiler version |
| `google_fonts` | Vega-Lite and Vega | Makes selected Google Fonts available to one conversion |
| `vega_plugin` | Vega-Lite and Vega | Registers plugin code for one conversion |
| `fullscreen` | Vega Editor URLs | Selects the full-screen editor URL form |

SVG input conversions accept only the raster options `scale`, `ppi`, and `quality`. Chart options such as `theme`, `width`, and `height` do not apply to them.

The examples override this Vega-Lite input:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/quick-start.vl.json
:language: json
```
:::

::::{interface} python
Pass overrides as keyword arguments:

```python
from pathlib import Path

import vl_convert as vlc

spec = Path("chart.vl.json").read_text(encoding="utf-8")
png = vlc.vegalite_to_png(
    spec,
    scale=2,
    width=640,
    height=360,
    theme="dark",
)
Path("chart.png").write_bytes(png)
```

A per-call `vega_plugin` also requires `allow_per_request_plugins=True` in `configure()`.
::::

::::{interface} rust
Put chart options in `VlOpts` or `VgOpts` and format options in an output type such as `PngOpts` or `HtmlOpts`:

```rust
use vl_convert_rs::{PngOpts, VlConverter, VlOpts};

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::new();
let output = converter
    .vegalite_to_png(
        spec,
        VlOpts {
            width: Some(640.0),
            height: Some(360.0),
            theme: Some("dark".to_string()),
            ..Default::default()
        },
        PngOpts {
            scale: Some(2.0),
            ..Default::default()
        },
    )
    .await?;
std::fs::write("chart.png", output.data)?;
```

A per-call plugin also requires `allow_per_request_plugins: true` in `VlcConfig`.
::::

::::{interface} server
Put overrides beside `spec` in the request body. Save this body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/conversion-overrides.json
:language: json
```
:::

Send it to the PNG endpoint:

```console
$ curl http://127.0.0.1:3000/vegalite/png \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.png
```

Per-request `google_fonts` requires `--allow-google-fonts`, and per-request `vega_plugin` requires `--allow-per-request-plugins`. The server rejects unknown fields, so a misspelled option returns an error instead of being ignored.
::::

::::{interface} cli
Pass overrides as options after the conversion command:

```console
$ vl-convert vl2png \
>   --input chart.vl.json \
>   --output chart.png \
>   --scale 2 \
>   --width 640 \
>   --height 360 \
>   --theme dark
```

Run a command with `--help` to see its options. `--config` is available on the Vega-Lite and Vega chart commands except `vl2url` and `vg2url`, which only encode the input specification in a URL. Converter settings such as `--google-font` and `--vega-plugin` are global options and can appear before or after the command.
::::
