---
title: Logging
path: guides/logging
section: Guides
order: 300
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Logging and Warnings

Warnings from Vega and Vega-Lite flow through each surface's logging channel.
CLI and server logging use Rust `tracing` targets. `--log-level` sets the
default vl-convert filter, while `--log-filter` or `VLC_LOG_FILTER` accepts a
raw `tracing_subscriber` filter such as `vl_convert=debug,tower_http=info`.

::::{interface} python
```python
import logging

logging.getLogger("vl_convert").setLevel(logging.WARNING)
```
::::

::::{interface} cli
```bash
vl-convert --log-level warn vl2svg --input chart.vl.json --output chart.svg
vl-convert --log-filter 'vl_convert=debug' \
  vl2svg --input chart.vl.json --output chart.svg
```
::::


::::{interface} rust
```bash
RUST_LOG=vl_convert=warn cargo run
```
::::


::::{interface} server
```bash
vl-convert serve --log-format=json --log-level=info
```

See {doc}`/server/logging` for request fields.
::::
