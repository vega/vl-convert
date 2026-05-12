---
title: Python Async API
path: advanced/python-async
section: Advanced
order: 420
interfaces: [python]
---

<!-- topic-body -->

# Python Async API

The `vl_convert.asyncio` module mirrors the sync Python API with awaitable
functions.

```python
import asyncio
import vl_convert.asyncio as vlca

async def main():
    await vlca.configure(num_workers=4)
    png = await vlca.vegalite_to_png(vl_spec)
    return png

png = asyncio.run(main())
```
