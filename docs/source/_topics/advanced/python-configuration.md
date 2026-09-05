---
title: Python Process Lifecycle
path: advanced/python-configuration
section: Advanced
order: 410
interfaces: [python]
---

<!-- topic-body -->

# Python Process Lifecycle

The Python package keeps one converter for the current process. Conversion
functions create its worker pool lazily on first use. Configure the process
once during application startup so later calls share the intended settings and
workers.

Use `load_config()` when a file owns the full configuration. It starts from
built-in defaults and then applies the JSONC file. Use `configure()` to change
only the named fields.

```python
import vl_convert as vlc

vlc.load_config("production.vlc.jsonc")
vlc.configure(
    num_workers=4,
    max_v8_heap_size_mb=1024,
)
```

The order matters in this example. Calling `load_config()` after `configure()`
would replace the earlier values.

## Warm Workers Before Serving Traffic

Workers normally start on the first conversion. Warm them during application
startup when first-request latency matters:

```python
vlc.configure(num_workers=4)
vlc.warm_up_workers()
```

`get_config()` returns the active converter settings, and
`get_worker_memory_usage()` reports current JavaScript heap statistics.

## Change Configuration Safely

A configuration change creates a replacement converter when any field differs.
Conversions already in progress can finish with the previous settings. New
calls use the replacement and start its workers lazily.

Frequent reconfiguration therefore discards the benefit of a warm worker pool.
Use per-call options for values such as scale, dimensions, theme, locale, and
output bundling. Reserve `configure()` for process-level policy and defaults.

`vl_convert.asyncio` uses the same converter state as the synchronous module.
Do not run competing configuration changes while other startup code is
configuring the process. See {doc}`python-async` for awaitable conversions and
{doc}`configuration` for every configuration field.
