---
title: Image Size and Quality
path: guides/image-quality
section: Output and Appearance
order: 230
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Image Size and Quality Recommendations

Prefer vector output when the consumer supports it. SVG works well in web pages and supports later manual editing. PDF works well for printing directly, and for embedding in LaTeX or Typst documents. Neither needs a fixed pixel size.

Use PNG when the consumer needs a high quality raster format. Use JPEG when a smaller file matters more than image quality.

## Chart Size

The specification's `width` and `height` control chart layout, not necessarily the final exported size. With the default `autosize: "pad"`, they size the plotting area, and Vega expands the output to include axes, legends, titles, and padding.

To request fixed outer dimensions, set `autosize` to `{"type": "fit", "contains": "padding"}` in the specification. This shrinks the plotting area to make room for the surrounding content and includes padding within the requested size. See [Vega autosize](https://vega.github.io/vega/docs/specification/#autosize) and [Vega-Lite sizing](https://vega.github.io/vega-lite/docs/size.html#autosize) for supported layouts, fitting limitations, and other sizing modes.

VlConvert exports the resulting layout. A chart fitted to 640 × 360 has those dimensions in SVG and produces a 640 × 360 PNG at the default `scale=1` and `ppi=72`. With `scale=2`, the same layout produces a 1280 × 720 PNG. The pixel-density settings below change the raster resolution without changing the layout.

## Pixel Density

`scale` multiplies the pixel dimensions of PNG and JPEG output without changing the layout. `2` is a common choice for high-density displays.

PNG also accepts `ppi`, which records pixels-per-inch metadata in the file and contributes to the pixel dimensions:

```text
effective scale = scale * ppi / 72
```

The defaults are `scale=1` and `ppi=72`. Either `scale=2` or `ppi=144` doubles the pixel dimensions. Set both only when you want the effects multiplied.

Some consumers use the `ppi` metadata to preserve the image's physical size when you increase `ppi` for higher resolution without changing `scale`.

## JPEG Quality

`quality` ranges from `0` through `100` and defaults to `90`. Higher values keep more detail and produce larger files.

## Examples

The examples use the input from Quick Start:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/quick-start.vl.json
:language: json
```
:::

::::{interface} python
```python
from pathlib import Path

import vl_convert as vlc

spec = Path("chart.vl.json").read_text(encoding="utf-8")
png = vlc.vegalite_to_png(spec, width=640, height=360, scale=2)
jpeg = vlc.vegalite_to_jpeg(spec, width=640, height=360, quality=90)
Path("chart.png").write_bytes(png)
Path("chart.jpg").write_bytes(jpeg)
```

Both functions return bytes. They also accept the specification as a dictionary.
::::

::::{interface} cli
```console
$ vl-convert vl2png \
>   --input chart.vl.json --output chart.png \
>   --width 640 --height 360 --scale 2

$ vl-convert vl2jpeg \
>   --input chart.vl.json --output chart.jpg \
>   --width 640 --height 360 --quality 90
```
::::

::::{interface} rust
```rust
use vl_convert_rs::{JpegOpts, PngOpts, VlConverter, VlOpts};

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::new();
let chart_size = VlOpts {
    width: Some(640.0),
    height: Some(360.0),
    ..Default::default()
};

let png = converter
    .vegalite_to_png(
        spec.clone(),
        chart_size.clone(),
        PngOpts {
            scale: Some(2.0),
            ..Default::default()
        },
    )
    .await?;

let jpeg = converter
    .vegalite_to_jpeg(
        spec,
        chart_size,
        JpegOpts {
            quality: Some(90),
            ..Default::default()
        },
    )
    .await?;
std::fs::write("chart.png", png.data)?;
std::fs::write("chart.jpg", jpeg.data)?;
```
::::

::::{interface} server
Put the options beside `spec` in the request body. For example, save this body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/image-quality-png.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/vegalite/png \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.png
```
::::

PNG output uses Vega's canvas renderer. JPEG and PDF are produced from the SVG rendering. SVG input is converted directly by Rust image and PDF libraries without running Vega, and `width` and `height` do not apply to it. See {doc}`../advanced/conversion-overrides` for every per-conversion option.
