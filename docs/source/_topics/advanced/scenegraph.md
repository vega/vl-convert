---
title: Scenegraph Output
path: advanced/scenegraph
section: Advanced
order: 470
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Evaluated Scenegraph Output

A Vega scenegraph is the evaluated tree of groups, marks, and text with their
computed positions and styles. It holds layout results rather than chart
instructions. Use it when another system needs to inspect rendered marks or
drive a custom renderer. Use SVG, PNG, JPEG, or PDF for ordinary image export.

The scenegraph is available as JSON or as MessagePack, a compact binary
encoding of the same structure.

::::{interface} python
```python
import vl_convert as vlc

scenegraph = vlc.vegalite_to_scenegraph(spec)
scenegraph_msgpack = vlc.vegalite_to_scenegraph(spec, format="msgpack")
```

The default result is a dictionary. The MessagePack result is bytes. Use
`vega_to_scenegraph()` for an already compiled Vega specification.
::::

::::{interface} cli
```bash
vl-convert vl2sg \
  --input chart.vl.json \
  --output scenegraph.json \
  --pretty

vl-convert vg2sg \
  --input chart.vg.json \
  --output scenegraph.msgpack \
  --format msgpack
```

`vl2sg` takes Vega-Lite input and `vg2sg` takes Vega input.
::::

::::{interface} rust
Separate methods return JSON and MessagePack:

```rust
let json_output = converter
    .vegalite_to_scenegraph(spec.clone(), Default::default())
    .await?;

let msgpack_output = converter
    .vegalite_to_scenegraph_msgpack(spec, Default::default())
    .await?;
```

Read `json_output.scenegraph` or `msgpack_output.data`. Both outputs also carry
Vega's diagnostic messages in `logs`.
::::

::::{interface} server
Send the normal JSON request body to `POST /vegalite/scenegraph` or
`POST /vega/scenegraph`. The response is JSON unless the `Accept` header asks
for MessagePack:

```bash
curl http://127.0.0.1:3000/vegalite/scenegraph \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/msgpack' \
  --data-binary @request.json \
  --output scenegraph.msgpack
```
::::

The scenegraph follows Vega's internal representation and can change when Vega
changes its layout or mark implementation. Keep the source specification as the
durable artifact.
