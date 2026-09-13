---
title: Converting from Vega-Lite
path: guides/vegalite-conversions
section: Convert and Export
order: 200
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Converting from Vega-Lite

Use the Vega-Lite functions for charts authored in Vega-Lite. VlConvert compiles the specification to Vega with the bundled Vega-Lite library, then returns or renders the result.

Available outputs: compiled Vega JSON, SVG, PNG, JPEG, PDF, HTML, a Vega Editor URL, an evaluated scenegraph. SVG, HTML, and URL outputs are text. PNG, JPEG, and PDF outputs are bytes.

VlConvert bundles several Vega-Lite versions and uses the newest by default. See [Supported Vega-Lite Versions](#supported-vega-lite-versions) to select another compiler. The setting does not change the input's `$schema` field.

The examples below use the same input from Quick Start:

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
vega_spec = vlc.vegalite_to_vega(spec)
svg = vlc.vegalite_to_svg(spec)
png = vlc.vegalite_to_png(spec, scale=2)
pdf = vlc.vegalite_to_pdf(spec)
Path("chart.svg").write_text(svg, encoding="utf-8")
Path("chart.png").write_bytes(png)
Path("chart.pdf").write_bytes(pdf)
```

The functions also accept the specification as a dictionary.
::::

::::{interface} cli
```console
$ vl-convert vl2svg --input chart.vl.json --output chart.svg
$ vl-convert vl2png --input chart.vl.json --output chart.png --scale 2
$ vl-convert vl2pdf --input chart.vl.json --output chart.pdf
$ vl-convert vl2vg --input chart.vl.json --output chart.vg.json --pretty
```

The other commands include `vl2jpeg`, `vl2html`, `vl2url`, `vl2sg`, and `vl2fonts`. Run any command with `--help` to see its options.
::::

::::{interface} rust
```rust
use vl_convert_rs::{VlConverter, VlOpts};

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::new();
let svg = converter
    .vegalite_to_svg(spec.clone(), VlOpts::default(), Default::default())
    .await?;
let png = converter
    .vegalite_to_png(spec, VlOpts::default(), Default::default())
    .await?;
std::fs::write("chart.svg", svg.svg)?;
std::fs::write("chart.png", png.data)?;
```

Read the result from `svg.svg` or `png.data`. Every output struct also carries Vega's diagnostic messages in `logs`.
::::

::::{interface} server
Send a request body to the endpoint for the output you need:

| Output | Endpoint |
| --- | --- |
| Compiled Vega | `POST /vegalite/vega` |
| SVG | `POST /vegalite/svg` |
| PNG | `POST /vegalite/png` |
| JPEG | `POST /vegalite/jpeg` |
| PDF | `POST /vegalite/pdf` |
| HTML | `POST /vegalite/html` |
| Vega Editor URL | `POST /vegalite/url` |
| Scenegraph | `POST /vegalite/scenegraph` |
| Resolved fonts | `POST /vegalite/fonts` |

For example, save this complete SVG request as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/vegalite-svg.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/vegalite/svg \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.svg
```

The response body contains the output directly. See {doc}`../api-reference` for the other request fields and response content types.
::::

## Supported Vega-Lite Versions

This VlConvert release accepts these major/minor versions, listed from oldest to newest. The newest is the default:

```{program-output} python -c "import vl_convert as vlc; print(', '.join(vlc.get_vegalite_versions()))"
```

Each selects the bundled patch release for that major/minor version. A full version string such as `6.4.1` selects the same compiler as `6.4`, not that exact patch release.

::::{interface} python
Pass `vl_version="6.4"` to a Vega-Lite conversion function. Use `vlc.get_vegalite_versions()` to list the versions in your installed package.
::::

::::{interface} cli
Pass `--vl-version 6.4` to a Vega-Lite conversion command. Its `--help` output lists the supported versions.
::::

::::{interface} rust
Set `VlOpts.vl_version` to a `VlVersion` variant, such as `VlVersion::v6_4`, or parse a version string with `"6.4.1".parse::<VlVersion>()?`. The crate-root `VL_VERSIONS` constant lists the bundled versions.
::::

::::{interface} server
Set `"vl_version": "6.4"` in the request body. `GET /info` returns the server's supported versions in `vegalite_versions`.
::::

Related guides: {doc}`data-loading` for specifications that load data from URLs or files, {doc}`image-quality` for size and resolution options, {doc}`themes` and {doc}`locales` for appearance and formatting, {doc}`html-output` for interactive pages, and {doc}`../advanced/scenegraph` and {doc}`../advanced/font-introspection` for the intermediate outputs.
