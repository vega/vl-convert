---
title: Vega Plugins
path: guides/plugins
section: Guides
order: 280
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Vega Plugins

A Vega plugin adds a named capability to the Vega runtime, such as an
expression function, color scheme, projection, scale, transform, or data
format. Most charts do not need a plugin. Use normal Vega or Vega-Lite
transforms, themes, configuration, and locales when they provide the behavior
you need.

Use a plugin when a specification refers to a custom runtime name. The
[Vega extensibility API](https://vega.github.io/vega/docs/api/extensibility/)
lists the available registration functions.

:::{warning}
A plugin is executable JavaScript. Load plugin files and URLs only from sources
you trust. Do not enable caller-supplied plugins on a public service unless you
intend to accept this risk and have strict resource limits.
:::

## Create a Plugin

A plugin is a JavaScript ECMAScript module (ESM) whose default export is a
function. VlConvert passes the Vega module to that function before it parses
the specification.

Save this example as `double-value.js`:

```javascript
export default function registerDoubleValue(vega) {
  vega.expressionFunction("doubleValue", value => value * 2)
}
```

Save this Vega-Lite specification as `chart.vl.json`:

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {"values": [{"category": "A", "value": 2}, {"category": "B", "value": 5}]},
  "transform": [{"calculate": "doubleValue(datum.value)", "as": "doubled"}],
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "doubled", "type": "quantitative"}
  }
}
```

The plugin registers `doubleValue` before Vega-Lite compiles the `calculate`
expression.

## Register the Plugin

::::{interface} python
Register a startup plugin with `configure()`, then convert specifications as
usual:

```python
import json
import vl_convert as vlc

vlc.configure(vega_plugins=["./double-value.js"])

with open("chart.vl.json", encoding="utf-8") as input_file:
    spec = json.load(input_file)

png = vlc.vegalite_to_png(spec)
with open("chart.png", "wb") as output_file:
    output_file.write(png)
```

The plugin remains available to later conversions in the Python process.
::::

::::{interface} cli
Pass the plugin as a global option before the conversion subcommand:

```bash
vl-convert --vega-plugin ./double-value.js \
  vl2png --input chart.vl.json --output chart.png
```

The plugin is available for this command invocation.
::::

::::{interface} rust
Register startup plugins in `VlcConfig`:

```rust
use vl_convert_rs::{PngOpts, VlcConfig, VlConverter, VlOpts};

let converter = VlConverter::with_config(VlcConfig {
    vega_plugins: vec!["./double-value.js".to_string()],
    ..Default::default()
})?;

let output = converter
    .vegalite_to_png(spec, VlOpts::default(), PngOpts::default())
    .await?;
std::fs::write("chart.png", output.data)?;
```

The example assumes `spec` contains the Vega-Lite value shown above.
::::

::::{interface} server
Register a startup plugin before `serve`:

```bash
vl-convert --vega-plugin ./double-value.js \
  serve --host 127.0.0.1 --port 3000
```

Every request handled by this process can then use `doubleValue`. Put the
Vega-Lite specification shown above in the `spec` field of a normal
`/vegalite/*` request. The quick start shows the complete HTTP request shape.
::::

Plugins can also come from HTTPS URLs or inline source, and a plugin can import
other modules. These choices affect startup behavior and network security. See
{doc}`../advanced/plugin-loading` for loading modes, import allowlists,
prebundling, and caller-supplied plugins.
