---
title: Python Async API
path: advanced/python-async
section: Advanced
order: 420
interfaces: [python]
---

<!-- topic-body -->

# Python Async API

Use `vl_convert.asyncio` when the caller already runs an asyncio event loop, such as an asynchronous web service. Its conversion and configuration functions are awaitable, so the event loop keeps serving other tasks while VlConvert works.

The synchronous and asynchronous modules share one converter and worker pool. Configure that shared state once at startup, before concurrent conversions begin.

```python
import asyncio
from pathlib import Path

import vl_convert.asyncio as vlca

spec = {
    "data": {"values": [{"category": "A", "value": 2}, {"category": "B", "value": 5}]},
    "mark": "bar",
    "encoding": {
        "x": {"field": "category", "type": "nominal"},
        "y": {"field": "value", "type": "quantitative"},
    },
}

async def main():
    await vlca.configure(num_workers=2)
    await vlca.warm_up_workers()

    png, svg = await asyncio.gather(
        vlca.vegalite_to_png(spec, scale=2),
        vlca.vegalite_to_svg(spec),
    )

    Path("chart.png").write_bytes(png)
    Path("chart.svg").write_text(svg, encoding="utf-8")

asyncio.run(main())
```

A pool runs as many conversions at once as it has workers, and extra calls wait for a free worker. Choose `num_workers` from measured concurrency and memory needs rather than from the number of tasks you submit. See {doc}`memory-management`.

Not everything in `vl_convert.asyncio` is awaitable. The version getters, `get_config_path()`, locale lookups, `current_font_directories()`, and Google Fonts cache helpers are synchronous re-exports. This includes `set_google_fonts_cache_size_mb()`. In contrast, `register_font_directory()` and `set_font_directories()` must be awaited. See the {doc}`../api-reference` for the full list of synchronous functions.
