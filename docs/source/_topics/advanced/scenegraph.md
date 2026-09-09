---
title: Scenegraph Output
path: advanced/scenegraph
section: Advanced
order: 470
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Evaluated Scenegraph Output

A Vega scenegraph is the evaluated tree of groups, marks, and text with their computed positions and styles. It holds layout results rather than chart instructions. Use it when another system needs to inspect rendered marks or drive a custom renderer. Use SVG, PNG, JPEG, or PDF for ordinary image export.

The scenegraph is available as JSON or as MessagePack, a compact binary encoding of the same structure.

The examples use the Vega-Lite specification from Quick Start and the direct Vega specification from {doc}`../guides/vega-conversions`:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/quick-start.vl.json
:language: json
```
:::

:::{dropdown} chart.vg.json
:open:

```{literalinclude} /_examples/vega-demo.vg.json
:language: json
```
:::

::::{interface} python
```python
import json
from pathlib import Path

import vl_convert as vlc

vl_spec = Path("chart.vl.json").read_text(encoding="utf-8")
vg_spec = Path("chart.vg.json").read_text(encoding="utf-8")
scenegraph = vlc.vegalite_to_scenegraph(vl_spec)
scenegraph_msgpack = vlc.vega_to_scenegraph(vg_spec, format="msgpack")
Path("scenegraph.json").write_text(
    json.dumps(scenegraph, indent=2),
    encoding="utf-8",
)
Path("scenegraph.msgpack").write_bytes(scenegraph_msgpack)
```

The default result is a dictionary. The MessagePack result is bytes. Use `vega_to_scenegraph()` for an already compiled Vega specification.
::::

::::{interface} cli
```console
$ vl-convert vl2sg \
>   --input chart.vl.json \
>   --output scenegraph.json \
>   --pretty

$ vl-convert vg2sg \
>   --input chart.vg.json \
>   --output scenegraph.msgpack \
>   --format msgpack
```

`vl2sg` takes Vega-Lite input and `vg2sg` takes Vega input.
::::

::::{interface} rust
Separate methods return JSON and MessagePack:

```rust
use vl_convert_rs::{serde_json, VgOpts, VlConverter, VlOpts};

let vl_spec = std::fs::read_to_string("chart.vl.json")?;
let vg_spec = std::fs::read_to_string("chart.vg.json")?;
let converter = VlConverter::new();
let json_output = converter
    .vegalite_to_scenegraph(vl_spec, VlOpts::default())
    .await?;

let msgpack_output = converter
    .vega_to_scenegraph_msgpack(vg_spec, VgOpts::default())
    .await?;

std::fs::write(
    "scenegraph.json",
    serde_json::to_vec_pretty(&json_output.scenegraph)?,
)?;
std::fs::write("scenegraph.msgpack", msgpack_output.data)?;
```

Read `json_output.scenegraph` or `msgpack_output.data`. Both outputs also carry Vega's diagnostic messages in `logs`.
::::

::::{interface} server
Send the normal JSON request body to `POST /vegalite/scenegraph` or `POST /vega/scenegraph`. The response is JSON unless the `Accept` header asks for MessagePack. Save this Vega-Lite body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/scenegraph-vegalite.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/vegalite/scenegraph \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output scenegraph.json
```

For the Vega and MessagePack combination, save this body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/scenegraph-vega.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/vega/scenegraph \
>   -H 'Content-Type: application/json' \
>   -H 'Accept: application/msgpack' \
>   --data-binary @request.json \
>   --output scenegraph.msgpack
```
::::

The scenegraph follows Vega's internal representation and can change when Vega changes its layout or mark implementation. Keep the source specification as the durable artifact.
