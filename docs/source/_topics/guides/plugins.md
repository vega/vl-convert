---
title: Vega Plugins
path: guides/plugins
section: Guides
order: 290
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Vega Plugins

A Vega plugin adds a named capability to the Vega runtime, such as an expression function, color scheme, projection, scale, transform, or data format. Most charts do not need one. Use a plugin only when a specification refers to a custom runtime name that Vega's built-in transforms, themes, configuration, and locales cannot provide. The [Vega extensibility API](https://vega.github.io/vega/docs/api/extensibility/) lists the registration functions a plugin can call.

:::{warning}
A plugin is executable JavaScript. Load plugin files and URLs only from sources you trust. Do not accept caller-supplied plugins on a public service unless you have decided to accept that risk and enforce strict resource limits.
:::

## Create a Plugin

A plugin is a JavaScript ECMAScript module (ESM) whose default export is a function. VlConvert calls that function with the Vega module when a worker starts, before any specification is compiled or parsed.

Save this example as `double-value.js`:

:::{dropdown} double-value.js
:open:

```{literalinclude} /_examples/double-value.js
:language: javascript
```
:::

Save this Vega-Lite specification:

:::{dropdown} chart.vl.json
:open:

```{literalinclude} /_examples/plugin-demo.vl.json
:language: json
```
:::

The `calculate` transform can now call `doubleValue`.

## Register the Plugin

::::{interface} python
Register the plugin with `configure()`, then convert as usual:

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

The plugin stays available to every later conversion in the process.
::::

::::{interface} cli
Pass the plugin as a global option before the conversion command:

```console
$ vl-convert --vega-plugin ./double-value.js \
>   vl2png --input chart.vl.json --output chart.png
```
::::

::::{interface} rust
List startup plugins in `VlcConfig`:

```rust
use vl_convert_rs::{PngOpts, VlcConfig, VlConverter, VlOpts};

let spec = std::fs::read_to_string("chart.vl.json")?;
let converter = VlConverter::with_config(VlcConfig {
    vega_plugins: vec!["./double-value.js".to_string()],
    ..Default::default()
})?;

let output = converter
    .vegalite_to_png(spec, VlOpts::default(), PngOpts::default())
    .await?;
std::fs::write("chart.png", output.data)?;
```
::::

::::{interface} server
Register the plugin before `serve`:

```console
$ vl-convert serve \
>   --vega-plugin ./double-value.js \
>   --port 3000
```

Every request handled by this process can then use `doubleValue`. Put the specification above in the `spec` field of a normal `/vegalite/*` request. Save this complete request body as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/plugin-startup-png.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/vegalite/png \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.png
```
::::

The registered `doubleValue` function doubles the source values before Vega draws these bars:

```{vl-chart} /_examples/plugin-demo.vl.json
:vega-plugin: /_examples/double-value.js
:alt: Two bars with the source values doubled to four and ten
```

Plugins can also come from HTTPS URLs or inline source, and a plugin can import other modules. See {doc}`../advanced/plugin-loading` for loading modes, import allowlists, prebundling, and caller-supplied plugins, and {doc}`security` for the wider trust model.
