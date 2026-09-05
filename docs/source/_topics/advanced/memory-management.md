---
title: Memory and Execution Limits
path: advanced/memory-management
section: Advanced
order: 480
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Memory and Execution Limits

VlConvert runs Vega and Vega-Lite in JavaScript workers. Two converter settings
bound that work:

- `max_v8_heap_size_mb` limits the JavaScript heap for each worker.
- `max_v8_execution_time_secs` stops JavaScript execution after the configured
  time.

Both are unlimited by default. Set them when specifications are untrusted or
when a service needs predictable failure boundaries.

These settings do not cap all process resources. Decoded images, fonts, raster
buffers, PDF generation, and other native allocations can use memory outside
the JavaScript heap. Use operating-system or container limits as the outer
boundary.

## Size the Worker Pool

Each persistent worker has its own JavaScript runtime and heap. More workers
allow more conversions to run at once but increase baseline and peak memory.
Start with the concurrency the application actually needs, then measure with
representative charts.

`gc_after_conversion` asks the JavaScript engine to collect garbage after every
conversion. It can reduce retained heap between requests, but it adds work and
can lower throughput. Enable it only after measurement shows a benefit.

::::{interface} python
Configure limits before the first conversion, then optionally warm the workers:

```python
import vl_convert as vlc

vlc.configure(
    num_workers=2,
    max_v8_heap_size_mb=512,
    max_v8_execution_time_secs=10,
    gc_after_conversion=True,
)
vlc.warm_up_workers()

for worker in vlc.get_worker_memory_usage():
    print(worker["worker_index"], worker["used_heap_size"])
```

Memory values are bytes. Calling `get_worker_memory_usage()` starts the workers
if they have not started yet.
::::

::::{interface} cli
A CLI conversion runs in a short-lived process, so heap and execution-time
limits are usually more useful than post-conversion garbage collection:

```bash
vl-convert \
  --max-v8-heap-size-mb 512 \
  --max-v8-execution-time-secs 10 \
  vl2png --input chart.vl.json --output chart.png
```
::::

::::{interface} rust
Set limits in `VlcConfig` and use `get_worker_memory_usage()` for current heap
statistics:

```rust
use std::num::NonZeroU64;
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    num_workers: NonZeroU64::new(2).unwrap(),
    max_v8_heap_size_mb: NonZeroU64::new(512),
    max_v8_execution_time_secs: NonZeroU64::new(10),
    ..Default::default()
})?;

converter.warm_up()?;
let usage = converter.get_worker_memory_usage().await?;
```
::::

::::{interface} server
Set converter limits before `serve` and combine them with HTTP request limits:

```bash
vl-convert \
  --max-v8-heap-size-mb 512 \
  --max-v8-execution-time-secs 10 \
  serve \
  --workers 2 \
  --max-concurrent-requests 4 \
  --request-timeout-secs 15
```

When the authenticated admin listener is enabled, inspect current JavaScript
heap use with:

```bash
curl http://127.0.0.1:3001/admin/diagnostics/workers \
  -H "Authorization: Bearer $ADMIN_API_KEY"
```

Render-time budgets can also limit how much shared service capacity one client
uses. See {doc}`/server/rate-limiting`.
::::

A timeout or heap-limit error stops the current conversion. Later conversions
can continue. If ordinary charts repeatedly hit a limit, reduce input size or
raise the boundary based on measured resource use.
