---
title: Vega Plugins
path: guides/plugins
section: Guides
order: 280
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Plugins

Vega plugins extend the JavaScript runtime with custom transforms,
expression functions, color schemes, scales, projections, and data formats.

::::{interface} python
```python
import vl_convert as vlc

vlc.configure(vega_plugins=["./plugin.js"])
```
::::

::::{interface} cli
```bash
vl-convert --vega-plugin ./plugin.js vl2svg \
  --input chart.vl.json --output chart.svg
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    vega_plugins: vec!["./plugin.js".to_string()],
    ..Default::default()
});
```
::::

Per-request plugins are a distinct code-execution surface. Enable them only
for trusted callers or isolated workloads.
