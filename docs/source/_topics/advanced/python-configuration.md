---
title: Python Process Lifecycle
path: advanced/python-configuration
section: Advanced
order: 410
interfaces: [python]
---

<!-- topic-body -->

# Python Process Lifecycle

The Python package keeps one converter for the whole process, and its worker pool starts on the first conversion. Configure the process once during application startup so every later call shares the intended settings and workers.

```python
import vl_convert as vlc

vlc.load_config("production.vlc.jsonc")
vlc.configure(
    num_workers=4,
    max_v8_heap_size_mb=1024,
)
```

Order matters here. `load_config()` replaces every setting, so calling it after `configure()` would discard the values `configure()` set. See {doc}`configuration` for both functions and the JSONC file format.

## Warm Workers Before Serving Traffic

Workers normally start on the first conversion. Start them during application startup when first-request latency matters:

```python
vlc.configure(num_workers=4)
vlc.warm_up_workers()
```

`get_worker_memory_usage()` reports the JavaScript heap statistics of each worker and starts the pool if it is not running yet.

## Change Configuration Safely

When a configuration call changes any field, VlConvert builds a replacement converter with a fresh worker pool. Conversions already running finish on the old pool, and new calls use the replacement, whose workers start lazily.

Frequent reconfiguration therefore throws away warm workers. Use per-call options for values such as scale, dimensions, theme, and locale, and reserve `configure()` for process-level policy. See {doc}`conversion-overrides`.

`vl_convert.asyncio` shares the same converter, so configure the process once before concurrent work begins. See {doc}`python-async`.
