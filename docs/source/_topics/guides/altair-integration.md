---
title: Altair Integration
path: guides/altair-integration
section: Guides
order: 230
interfaces: [python]
---

<!-- topic-body -->

# Using with Altair

Altair emits Vega-Lite specifications. Pass `chart.to_dict()` or
`chart.to_json()` to the Python API when a workflow needs explicit image bytes
or file output; `to_dict()` avoids a JSON string round trip.

```python
import altair as alt
import vl_convert as vlc

chart = alt.Chart(data).mark_bar().encode(x="category:N", y="value:Q")
png = vlc.vegalite_to_png(chart.to_dict(), scale=2)

with open("chart.png", "wb") as f:
    f.write(png)
```

Altair's built-in save support also uses vl-convert for several export paths.
