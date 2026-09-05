---
title: Scenegraph Output
path: advanced/scenegraph
section: Advanced
order: 450
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Evaluated Scenegraph Output

A Vega scenegraph is the evaluated hierarchy of groups, marks, text, and their
computed properties. It contains layout results rather than the original chart
instructions. Use it when another system needs to inspect rendered marks or
build a custom renderer. Use SVG, PNG, JPEG, or PDF for normal image export.

JSON is convenient for inspection and interoperability. MessagePack carries the
same kind of result in a smaller binary representation.

::::{interface} python
```python
import vl_convert as vlc

scenegraph = vlc.vegalite_to_scenegraph(spec)
scenegraph_msgpack = vlc.vegalite_to_scenegraph(spec, format="msgpack")
```

The default result is a Python dictionary. The MessagePack result is bytes.
Use `vega_to_scenegraph()` for an already compiled Vega specification.
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

Use a `vl2sg` command for Vega-Lite input and `vg2sg` for Vega input.
::::

::::{interface} rust
Rust provides separate methods for structured JSON and MessagePack bytes:

```rust
let json_output = converter
    .vegalite_to_scenegraph(spec.clone(), Default::default())
    .await?;

let msgpack_output = converter
    .vegalite_to_scenegraph_msgpack(spec, Default::default())
    .await?;
```

Read `json_output.scenegraph` or `msgpack_output.data`. Both outputs also
include Vega diagnostic messages.
::::

::::{interface} server
Send the normal JSON request body to `POST /vegalite/scenegraph` or
`POST /vega/scenegraph`. JSON is the default response. Request MessagePack with
an `Accept` header:

```bash
curl http://127.0.0.1:3000/vegalite/scenegraph \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/msgpack' \
  --data-binary @request.json \
  --output scenegraph.msgpack
```
::::

The scenegraph follows Vega's evaluated representation and can change when Vega
changes its layout or mark implementation. Do not treat it as a substitute for
the source specification.
