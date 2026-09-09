---
title: Altair Integration
path: guides/altair-integration
section: Guides
order: 205
interfaces: [python]
---

<!-- topic-body -->

# Using VlConvert with Altair

Altair charts compile to Vega-Lite specifications, so any Altair chart can be rendered with the Vega-Lite functions. Pass `chart.to_dict()` directly. This skips serializing the specification to JSON and parsing it again.

Altair's own `chart.save()` already uses VlConvert to write static image files. Call VlConvert directly when you need options that `save()` does not expose, or when you want the result in memory instead of on disk.

Install both packages if the application does not already include Altair:

```console
$ uv add altair "vl-convert-python>=2.0.0rc1,<3"
```

This complete example creates a chart and writes a high-density PNG:

```python
import altair as alt
import vl_convert as vlc

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
)

png = vlc.vegalite_to_png(chart.to_dict(), scale=2)
with open("chart.png", "wb") as output_file:
    output_file.write(png)
```

Use `vegalite_to_svg()` for SVG text or `vegalite_to_pdf()` for PDF bytes. If a chart was produced by an older Altair release, pass `vl_version` to select the matching Vega-Lite compiler. See {doc}`vegalite-conversions` for the full list of outputs and {doc}`image-quality` for size and resolution options.
