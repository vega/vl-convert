---
title: Memory and Execution Limits
path: advanced/memory-management
section: Advanced
order: 450
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Memory and Execution Limits

VlConvert runs Vega and Vega-Lite in JavaScript workers. Two converter settings bound that work:

- `max_v8_heap_size_mb` caps the JavaScript heap of each worker. The minimum accepted value is 64 MB.
- `max_v8_execution_time_secs` stops JavaScript execution after the given number of seconds.

Both are unlimited by default. Set them when specifications are untrusted or when a service needs predictable failure boundaries. A conversion that hits either limit fails, and later conversions continue normally.

These settings do not cap the whole process. Decoded images, fonts, raster buffers, PDF generation, and other native allocations live outside the JavaScript heap. Use operating-system or container limits as the outer boundary.

## Size the Worker Pool

Each persistent worker has its own JavaScript runtime and heap. More workers allow more concurrent conversions but raise baseline and peak memory. Start with the concurrency the application needs, then measure with representative charts.

`gc_after_conversion` asks the JavaScript engine to collect garbage after every conversion. It can reduce the heap retained between requests at the cost of throughput. Enable it only after measurement shows a benefit.

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

Memory values are bytes. `get_worker_memory_usage()` starts the workers if they are not running yet.
::::

::::{interface} cli
A CLI conversion runs in a short-lived process, so heap and execution-time limits are more useful than post-conversion garbage collection. This example uses `chart.vl.json` from {doc}`../getting-started/quick-start`:

```console
$ vl-convert \
>   --max-v8-heap-size-mb 512 \
>   --max-v8-execution-time-secs 10 \
>   vl2png --input chart.vl.json --output chart.png
```
::::

::::{interface} rust
Set limits in `VlcConfig` and read heap statistics with `get_worker_memory_usage()`:

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
Combine converter limits with HTTP request limits when starting the server:

```console
$ vl-convert serve \
>   --max-v8-heap-size-mb 512 \
>   --max-v8-execution-time-secs 10 \
>   --workers 2 \
>   --max-concurrent-requests 4 \
>   --request-timeout-secs 15
```

When the admin listener is enabled, read the current JavaScript heap use with:

```console
$ curl http://127.0.0.1:3001/admin/diagnostics/workers \
>   -H "Authorization: Bearer $ADMIN_API_KEY"
```

Render-time budgets also limit how much shared capacity one client can use. See {doc}`/server/rate-limiting`.
::::

If ordinary charts keep hitting a limit, reduce the input size or raise the limit based on measured use. See {doc}`troubleshooting`.
