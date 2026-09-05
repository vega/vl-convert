---
title: Python Async API
path: advanced/python-async
section: Advanced
order: 420
interfaces: [python]
---

<!-- topic-body -->

# Python Async API

Use `vl_convert.asyncio` when the caller already runs an asyncio event loop,
such as an asynchronous web service. Its conversion and configuration functions
are awaitable, so the event loop can continue serving other tasks while
VlConvert works.

The synchronous and asynchronous modules share one process-wide converter
configuration and worker pool. Configure that shared state once during
application startup before concurrent conversions begin.

```python
import asyncio
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

    with open("chart.png", "wb") as png_file:
        png_file.write(png)
    with open("chart.svg", "w", encoding="utf-8") as svg_file:
        svg_file.write(svg)

asyncio.run(main())
```

A pool can run conversions concurrently up to its available workers. Extra
calls wait for a worker. Choose `num_workers` from measured concurrency and
memory needs rather than the number of submitted tasks.

Static metadata values and helpers that do not perform conversion remain
synchronous even when they are available from `vl_convert.asyncio`. Do not
`await` a function unless its API reference identifies it as awaitable.
