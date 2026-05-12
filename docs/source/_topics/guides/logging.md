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

::::{interface} python
```python
import logging

logging.getLogger("vl_convert").setLevel(logging.WARNING)
```
::::

::::{interface} cli
```bash
vl-convert --log-level warn vl2svg --input chart.vl.json --output chart.svg
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
