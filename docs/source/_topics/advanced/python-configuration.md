---
title: Python Configuration
path: advanced/python-configuration
section: Advanced
order: 410
interfaces: [python]
---

<!-- topic-body -->

# Python Configuration

Python keeps one process-wide converter configuration. `configure()` patches
selected fields, while `load_config()` replaces the active configuration from a
JSONC file using the same schema described in
{doc}`/python/advanced/configuration`.

```python
import vl_convert as vlc

vlc.configure(
    num_workers=4,
    auto_google_fonts=True,
    google_font_variant_threshold=16,
)
cfg = vlc.get_config()

vlc.load_config("production.vlc.jsonc")
```

Changing worker-affecting settings, such as `num_workers`,
`max_v8_heap_size_mb`, or `max_v8_execution_time_secs`, rebuilds the worker
pool. Use `warm_up_workers()` after startup configuration when first-request
latency matters:

```python
vlc.configure(num_workers=4, max_v8_heap_size_mb=1024)
vlc.warm_up_workers()
```

`vl_convert.asyncio` uses the same underlying converter state as the sync API.
Use the async namespace when the caller is already running an event loop; avoid
mixing sync and async configuration calls concurrently during application
startup.
