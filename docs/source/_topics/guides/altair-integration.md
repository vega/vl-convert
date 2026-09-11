---
title: Altair Integration
path: guides/altair-integration
section: Guides
order: 205
interfaces: [python]
---

<!-- topic-body -->

# Configure Altair Exports

Use Altair's `chart.save()` to export charts. Altair calls VlConvert for PNG, SVG, and PDF output, so you can use `vlc.configure()` to set up fonts and file access before saving. See [Saving Altair Charts](https://altair-viz.github.io/user_guide/saving_charts.html) for output formats, PNG resolution, and other `save()` options.

The configuration examples below require VlConvert 2.0.0rc6 or later:

```console
$ uv add altair "vl-convert-python>=2.0.0rc6,<3"
```

## Save with a Google Font

`vlc.configure(google_fonts=...)` makes a font available to the converter. Altair's `chart.configure(font=...)` selects that font for the chart. This example uses both to save a PNG and PDF:

```python
import altair as alt
import vl_convert as vlc

vlc.configure(
    google_fonts=["Roboto Slab"],
    missing_fonts="error",
)

data = alt.Data(
    values=[
        {"category": "A", "value": 2},
        {"category": "B", "value": 5},
        {"category": "C", "value": 3},
    ]
)

chart = (
    alt.Chart(data)
    .mark_bar()
    .encode(
        x=alt.X("category:N", title="Category"),
        y=alt.Y("value:Q", title="Value"),
    )
    .configure(font="Roboto Slab")
)

chart.save("chart.png", ppi=200)
chart.save("chart.pdf")
```

VlConvert downloads the font on first use and caches it for later conversions. `missing_fonts="error"` fails the export if a first-choice font is unavailable, instead of substituting another font. The PNG contains rendered pixels, and the PDF embeds its fonts, so recipients do not need to install Roboto Slab.

Configure VlConvert once before exporting. The settings apply to subsequent conversions throughout the Python process, including calls made by Altair. They are not stored in the chart specification. See {doc}`../advanced/python-configuration` for details.

Altair forwards only selected options to VlConvert. Set `google_fonts` through `vlc.configure()`, not as an argument to `chart.save()`.

### Local Fonts and Automatic Downloads

VlConvert also uses fonts installed on the host. To use an additional font directory, register its absolute path with `vlc.register_font_directory()` before saving. Continue to select the family in the Altair chart.

For trusted charts, `vlc.configure(auto_google_fonts=True)` lets VlConvert look up missing first-choice fonts in Google Fonts. This requires network access when the font is not cached. See {doc}`fonts` for local font registration, caching, and embedding options.

## Allow Local Data and Images

VlConvert blocks local file access by default. For charts that reference files in an existing `data` directory, configure that directory as the base for relative URLs and allow VlConvert to read it:

```python
from pathlib import Path

data_dir = str(Path("data").resolve())
vlc.configure(base_url=data_dir, allowed_base_urls=[data_dir])
```

Subsequent `chart.save()` calls use these settings for files loaded by VlConvert. This allowlist replaces the default HTTP and HTTPS access, so include any required remote prefixes too. DataFrames passed directly to Altair do not need local file access. See {doc}`data-loading` for complete examples and access rules.

## HTML and SVG Limitations

Altair does not yet expose all of VlConvert's HTML and SVG export features:

- `chart.save("chart.html")` uses Altair's HTML templates, not VlConvert's HTML converter. Even with `inline=True`, Altair uses VlConvert only to bundle JavaScript. It does not include the fonts configured above or transfer VlConvert's file-access settings to the browser.
- `chart.save("chart.svg")` delegates rendering to VlConvert but does not forward `bundle=True`. Google Fonts remain external references in the SVG, which some viewers cannot load.

Call VlConvert directly when you need HTML with embedded fonts or SVG with embedded fonts and images. The following example reuses the Google Font chart above and its converter configuration.

First, check the active data transformer. [VegaFusion](https://altair-viz.github.io/user_guide/large_datasets.html#vegafusion-data-transformer) requires `chart.to_dict(format="vega")` and the `vega_to_*` functions. Otherwise, use a Vega-Lite specification and the `vegalite_to_*` functions:

```python
from pathlib import Path

if alt.data_transformers.active == "vegafusion":
    spec = chart.to_dict(format="vega")
    html = vlc.vega_to_html(spec, bundle=True)
    svg = vlc.vega_to_svg(spec, bundle=True)
else:
    with alt.data_transformers.enable("default", max_rows=None):
        spec = chart.to_dict()
    html = vlc.vegalite_to_html(spec, vl_version=alt.VEGALITE_VERSION, bundle=True)
    svg = vlc.vegalite_to_svg(spec, vl_version=alt.VEGALITE_VERSION, bundle=True)

Path("chart.html").write_text(html, encoding="utf-8")
Path("chart.svg").write_text(svg, encoding="utf-8")
```

The Vega-Lite branch temporarily selects Altair's default transformer to inline data without its row limit, then restores the previous transformer. Passing `alt.VEGALITE_VERSION` selects the bundled compiler for Altair's major/minor version. The patch component is ignored, so `"6.4.1"` selects the bundled 6.4 compiler, not necessarily patch 6.4.1.

`bundle=True` embeds the Google Font in both outputs and the JavaScript libraries in HTML. HTML can still reference external data or images, so bundling alone does not make every chart work offline. See {doc}`html-output` and {doc}`fonts` for details.
