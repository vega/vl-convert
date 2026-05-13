---
title: Scenegraph Output
path: advanced/scenegraph
section: Advanced
order: 450
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Scenegraph Output

Scenegraph output exposes Vega's evaluated scenegraph as JSON or MessagePack.

::::{interface} python
```python
scenegraph = vlc.vegalite_to_scenegraph(vl_spec)
```
::::

::::{interface} cli
```bash
vl-convert vl2sg --input chart.vl.json --output scenegraph.json
vl-convert vg2sg --format msgpack --input chart.vg.json --output scenegraph.msgpack
```
::::


::::{interface} rust
Rust exposes separate methods for JSON and MessagePack scenegraphs.

```rust
let json = converter.vegalite_to_scenegraph(spec.clone(), Default::default()).await?;
let msgpack = converter
    .vegalite_to_scenegraph_msgpack(spec, Default::default())
    .await?;
```
::::


::::{interface} server
```bash
jq -c '{spec: .}' chart.vl.json |
  curl -X POST http://localhost:3000/vegalite/scenegraph \
  -H 'Content-Type: application/json' \
  --data-binary @- > scenegraph.json
```
::::
