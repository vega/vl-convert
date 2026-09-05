---
title: Conversion Overrides
path: advanced/conversion-overrides
section: Advanced
order: 407
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Conversion Overrides

Use an override when one conversion needs a value that differs from the
converter default. Persistent settings belong in converter configuration. See
{doc}`configuration` when the same value should apply to later conversions.

| Override | Used By | Effect |
| --- | --- | --- |
| `theme` | Vega-Lite | Applies a built-in or custom Vega-Lite theme |
| `config` | Vega-Lite and supported Vega conversions | Merges Vega or Vega-Lite configuration into the input |
| `format_locale` | Vega-Lite and Vega | Selects number formatting rules |
| `time_format_locale` | Vega-Lite and Vega | Selects date and time formatting rules |
| `background`, `width`, `height` | Vega-Lite and Vega | Changes the chart before rendering or evaluation |
| `scale` | PNG and JPEG | Multiplies raster pixel dimensions |
| `ppi` | PNG | Combines with `scale` as `scale * ppi / 72` |
| `quality` | JPEG | Sets encoding quality from `0` through `100`, with a default of `90` |
| `bundle` | SVG and HTML | Embeds assets in SVG or browser dependencies in HTML |
| `renderer` | HTML | Selects `svg`, `canvas`, or `hybrid` rendering in the browser |
| `vl_version` | Vega-Lite | Selects a bundled Vega-Lite compiler version |
| `google_fonts` | Vega-Lite and Vega | Makes selected Google Fonts available to one conversion |
| `vega_plugin` | Vega-Lite and Vega | Registers plugin code for one conversion |
| `fullscreen` | Vega Editor URLs | Selects the full-screen editor URL form |

SVG input conversions have their own format options and do not use chart
options such as `theme`, `width`, or `height`.

::::{interface} python
Pass overrides as keyword arguments to the conversion function:

```python
png = vlc.vegalite_to_png(
    spec,
    scale=2,
    width=640,
    height=360,
    theme="dark",
)
```

Python supports `config` for both Vega-Lite and Vega conversions. A per-call
`vega_plugin` also requires `allow_per_request_plugins=True` in `configure()`.
::::

::::{interface} rust
Put chart options in `VlOpts` or `VgOpts` and format options in an output type
such as `PngOpts` or `HtmlOpts`:

```rust
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
```

A per-call plugin also requires `allow_per_request_plugins: true` in
`VlcConfig`.
::::

::::{interface} server
Put overrides beside `spec` in the JSON request body:

```json
{
  "spec": {"mark": "bar", "data": {"values": []}},
  "scale": 2,
  "width": 640,
  "height": 360,
  "theme": "dark"
}
```

Per-request Google Fonts require `--allow-google-fonts`. Per-request plugin
code requires `--allow-per-request-plugins`. The server rejects unknown JSON
fields, so a misspelled option causes an error instead of being ignored.
::::

::::{interface} cli
Pass overrides as flags after the conversion subcommand:

```bash
vl-convert vl2png \
  --input chart.vl.json \
  --output chart.png \
  --scale 2 \
  --width 640 \
  --height 360 \
  --theme dark
```

Run the subcommand with `--help` to see its supported overrides. The CLI
supports `--config` on Vega-Lite commands but not on Vega commands. Global
settings such as `--google-font` and `--vega-plugin` appear before the
subcommand and configure the converter for the command invocation.
::::
