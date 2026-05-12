---
title: Altair Integration
path: guides/altair-integration
section: Guides
order: 230
interfaces: [python]
---

<!-- topic-body -->

# Using with Altair

Altair emits Vega-Lite JSON. Pass `chart.to_json()` to the Python API when a
workflow needs explicit image bytes or file output.

```python
import altair as alt
import vl_convert as vlc

chart = alt.Chart(data).mark_bar().encode(x="category:N", y="value:Q")
png = vlc.vegalite_to_png(chart.to_json(), scale=2)

with open("chart.png", "wb") as f:
    f.write(png)
```

Altair's built-in save support also uses vl-convert for several export paths.
