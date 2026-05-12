---
title: Memory Management
path: advanced/memory-management
section: Advanced
order: 480
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Memory Management

Use heap limits, post-conversion GC, and worker diagnostics to keep long-lived
processes predictable.

::::{interface} python
```python
vlc.configure(max_v8_heap_size_mb=512, gc_after_conversion=True)
usage = vlc.get_worker_memory_usage()
```
::::

::::{interface} cli
```bash
vl-convert --max-v8-heap-size-mb 512 --gc-after-conversion \
  vl2png --input chart.vl.json --output chart.png
```
::::


::::{interface} rust
```rust
let usage = converter.get_worker_memory_usage().await?;
```
::::


::::{interface} server
```bash
curl http://localhost:3001/admin/diagnostics/workers
```
::::
