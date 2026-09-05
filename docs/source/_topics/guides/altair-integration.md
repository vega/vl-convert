---
title: Altair Integration
path: guides/altair-integration
section: Guides
order: 230
interfaces: [python]
---

<!-- topic-body -->

# Using VlConvert with Altair

Altair charts produce Vega-Lite specifications. Pass `chart.to_dict()` to
VlConvert when an application needs the rendered bytes or text in memory.
Using a dictionary avoids converting the specification to JSON and parsing it
again.

Install both packages if the application does not already include Altair:

```bash
python -m pip install altair vl-convert-python
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

Use `vegalite_to_svg()` for SVG text or `vegalite_to_pdf()` for PDF bytes.
Altair's own save support also uses VlConvert for supported export paths. Call
VlConvert directly when you need its conversion options or want to keep the
result in memory.
