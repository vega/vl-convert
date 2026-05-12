---
title: Vega Plugins
path: guides/plugins
section: Guides
order: 280
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Vega Plugins

Use a Vega plugin when a spec needs a Vega extension that is not part of the
standard Vega runtime. Plugins can register expression functions, color
schemes, projections, scales, transforms, and data formats.
Vega's registry functions are documented in the
[Vega extensibility API](https://vega.github.io/vega/docs/api/extensibility/).

Most charts do not need plugins. Prefer normal Vega/Vega-Lite transforms,
themes, config, locales, and font settings when those are enough. Reach for a
plugin when the spec itself refers to a custom runtime name, such as a custom
expression function in a `calculate` transform or a custom color scheme in a
scale.

A plugin is a JavaScript ESM module with a default export. VlConvert calls that
function with the Vega module object before parsing specs that use the plugin.

```javascript
export default function registerPlugin(vega) {
  vega.scheme("acmeRamp", ["#225ea8", "#41b6c4", "#a1dab4", "#ffffcc"]);

  vega.expressionFunction("tierLabel", (name, value) => {
    const tier = value >= 10 ? "high" : "base";
    return `${name} (${tier})`;
  });
}
```

## Complete Example

Save the plugin as `acme-vega-plugin.js`.

```javascript
export default function registerAcmePlugin(vega) {
  vega.scheme("acmeRamp", ["#225ea8", "#41b6c4", "#a1dab4", "#ffffcc"]);

  vega.expressionFunction("tierLabel", (name, value) => {
    const tier = value >= 10 ? "high" : "base";
    return `${name} (${tier})`;
  });
}
```

Save this Vega-Lite spec as `chart.vl.json`.

```json
{
  "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
  "data": {
    "values": [
      {"category": "Alpha", "value": 6},
      {"category": "Beta", "value": 14},
      {"category": "Gamma", "value": 9}
    ]
  },
  "transform": [
    {
      "calculate": "tierLabel(datum.category, datum.value)",
      "as": "label"
    }
  ],
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "value", "type": "quantitative"},
    "color": {
      "field": "value",
      "type": "quantitative",
      "scale": {"scheme": "acmeRamp"}
    },
    "tooltip": [
      {"field": "label", "type": "nominal"},
      {"field": "value", "type": "quantitative"}
    ]
  }
}
```

::::{interface} python
Register the plugin once with `configure()`, then run conversions normally.

```python
import json
import vl_convert as vlc

vlc.configure(vega_plugins=["./acme-vega-plugin.js"])

with open("chart.vl.json") as f:
    spec = json.load(f)

png = vlc.vegalite_to_png(spec, scale=2)
with open("chart.png", "wb") as f:
    f.write(png)
```
::::

::::{interface} cli
Pass the plugin as a global CLI option. The `--vega-plugin` flag accepts file
paths and URLs. Put inline plugin source in a JSONC config file instead.

```bash
vl-convert --vega-plugin ./acme-vega-plugin.js \
  vl2png --input chart.vl.json --output chart.png --scale 2
```
::::

::::{interface} rust
Register the plugin in `VlcConfig`.

```rust
use vl_convert_rs::{PngOpts, VlConverter, VlOpts, VlcConfig};

let converter = VlConverter::with_config(VlcConfig {
    vega_plugins: vec!["./acme-vega-plugin.js".to_string()],
    ..Default::default()
})?;

let output = converter
    .vegalite_to_png(spec, VlOpts::default(), PngOpts { scale: Some(2.0), ppi: None })
    .await?;
std::fs::write("chart.png", output.data)?;
```
::::

::::{interface} server
Configure startup plugins on the server process. Every request handled by this
server can then use specs that reference the registered plugin names.

```bash
vl-convert --vega-plugin ./acme-vega-plugin.js \
  serve --host 127.0.0.1 --port 3000
```

```bash
python - <<'PY' > request.json
import json

with open("chart.vl.json") as f:
    print(json.dumps({"spec": json.load(f)}))
PY
```

```bash
curl -X POST http://127.0.0.1:3000/vegalite/png \
  -H 'Content-Type: application/json' \
  --data-binary @request.json > chart.png
```
::::

## Plugin Entry Forms

VlConvert accepts these plugin entry forms:

- A `.js` or `.mjs` file path. The file is read by VlConvert and bundled
  before the plugin is loaded. Local multi-file plugins should be bundled into
  one ESM file before passing them to VlConvert.
- An `http://` or `https://` URL. The URL is fetched and bundled by VlConvert.
- Inline ESM source. This is useful in config files and APIs that accept raw
  plugin source. The CLI flag rejects inline source so shell quoting errors do
  not silently become plugin code.

::::{interface} python
```python
vlc.configure(
    vega_plugins=[
        "./acme-vega-plugin.js",
        "https://cdn.example.com/acme-vega-plugin.js",
        "export default function(vega) { vega.expressionFunction('answer', () => 42); }",
    ]
)
```
::::

::::{interface} cli
```bash
vl-convert --vega-plugin ./acme-vega-plugin.js vl2svg \
  --input chart.vl.json --output chart.svg

vl-convert --vega-plugin https://cdn.example.com/acme-vega-plugin.js vl2svg \
  --input chart.vl.json --output chart.svg
```

For inline source, use a JSONC config file:

```json
{
  "vega_plugins": [
    "export default function(vega) { vega.expressionFunction('answer', () => 42); }"
  ]
}
```
::::

::::{interface} rust
```rust
let config = VlcConfig {
    vega_plugins: vec![
        "./acme-vega-plugin.js".to_string(),
        "https://cdn.example.com/acme-vega-plugin.js".to_string(),
        "export default function(vega) { vega.expressionFunction('answer', () => 42); }"
            .to_string(),
    ],
    ..Default::default()
};
```
::::

::::{interface} server
Startup plugins can come from CLI globals or from the server's `--vlc-config`
file. Inline source belongs in the config file.

```json
{
  "vega_plugins": [
    "./acme-vega-plugin.js",
    "https://cdn.example.com/acme-vega-plugin.js"
  ]
}
```
::::

## Imports and CDNs

Plugin code may import other ESM modules. HTTP imports are blocked unless their
domains are listed in `plugin_import_domains`.

```javascript
import { scaleLinear } from "https://esm.sh/d3-scale@4";

export default function registerScalePlugin(vega) {
  const scale = scaleLinear().domain([0, 1]).range([0, 100]);
  vega.expressionFunction("scaledPercent", value => scale(value));
}
```

::::{interface} python
```python
vlc.configure(
    vega_plugins=["./scale-plugin.js"],
    plugin_import_domains=["esm.sh"],
)
```
::::

::::{interface} cli
```bash
vl-convert \
  --vega-plugin ./scale-plugin.js \
  --plugin-import-domains esm.sh \
  vl2svg --input chart.vl.json --output chart.svg
```
::::

::::{interface} rust
```rust
let converter = VlConverter::with_config(VlcConfig {
    vega_plugins: vec!["./scale-plugin.js".to_string()],
    plugin_import_domains: vec!["esm.sh".to_string()],
    ..Default::default()
})?;
```
::::

::::{interface} server
```bash
vl-convert \
  --vega-plugin ./scale-plugin.js \
  --plugin-import-domains esm.sh \
  serve --host 127.0.0.1 --port 3000
```
::::

Domain patterns are exact by default. `esm.sh` allows only `esm.sh`;
`*.jsdelivr.net` allows `jsdelivr.net` and its subdomains; `*` allows any
domain. Redirect targets are checked against the same allowlist.

The plugin entry URL's own domain is allowed automatically. Imports from other
domains still need to be listed. For example, a plugin entry at
`https://cdn.example.com/acme-vega-plugin.js` can import relative modules from
`cdn.example.com`, but an import from `https://esm.sh/...` needs
`plugin_import_domains=["esm.sh"]`.

## Bundling Plugins Yourself

For production plugins, bundle TypeScript or multi-file JavaScript before
passing the plugin to VlConvert. This keeps startup deterministic and avoids
network fetches during plugin resolution.

```bash
npm install --save-dev esbuild
npx esbuild src/acme-vega-plugin.ts \
  --bundle \
  --format=esm \
  --platform=browser \
  --outfile=dist/acme-vega-plugin.js
```

Then point VlConvert at the bundled file:

::::{interface} python
```python
vlc.configure(vega_plugins=["dist/acme-vega-plugin.js"])
```
::::

::::{interface} cli
```bash
vl-convert --vega-plugin dist/acme-vega-plugin.js \
  vl2png --input chart.vl.json --output chart.png
```
::::

::::{interface} rust
```rust
let converter = VlConverter::with_config(VlcConfig {
    vega_plugins: vec!["dist/acme-vega-plugin.js".to_string()],
    ..Default::default()
})?;
```
::::

::::{interface} server
```bash
vl-convert --vega-plugin dist/acme-vega-plugin.js \
  serve --host 127.0.0.1 --port 3000
```
::::

::::{interface} python rust server
## Per-Request Plugins

Use startup plugins for trusted, stable extensions. Use per-request plugins
only when the caller must choose plugin code dynamically.

Per-request plugins are disabled by default. When enabled, each request with a
plugin runs on an ephemeral V8 worker. The plugin does not stay registered on
the main worker pool, but it still executes JavaScript supplied by the caller.
Apply the same trust boundary you would apply to other user-controlled code.
::::

::::{interface} python
```python
import vl_convert as vlc

vlc.configure(allow_per_request_plugins=True)

svg = vlc.vegalite_to_svg(
    spec,
    vega_plugin="export default function(vega) { vega.expressionFunction('answer', () => 42); }",
)
```
::::

::::{interface} rust
```rust
let converter = VlConverter::with_config(VlcConfig {
    allow_per_request_plugins: true,
    ..Default::default()
})?;

let output = converter
    .vegalite_to_svg(
        spec,
        VlOpts {
            vega_plugin: Some(
                "export default function(vega) { vega.expressionFunction('answer', () => 42); }"
                    .to_string(),
            ),
            ..Default::default()
        },
        Default::default(),
    )
    .await?;
```
::::

::::{interface} server
```bash
vl-convert serve \
  --host 127.0.0.1 \
  --port 3000 \
  --allow-per-request-plugins \
  --max-ephemeral-workers 2
```

```bash
curl -X POST http://127.0.0.1:3000/vegalite/svg \
  -H 'Content-Type: application/json' \
  --data '{
    "spec": {
      "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
      "data": {"values": [{"x": 1}]},
      "transform": [{"calculate": "answer()", "as": "y"}],
      "mark": "text",
      "encoding": {"text": {"field": "y", "type": "quantitative"}}
    },
    "vega_plugin": "export default function(vega) { vega.expressionFunction(\"answer\", () => 42); }"
  }'
```

If per-request plugins import from HTTP URLs, configure
`--per-request-plugin-import-domains`. This is separate from
`--plugin-import-domains`, which applies to startup plugins.
::::

## Runtime Behavior

VlConvert bundles each plugin before loading it into V8. Bundling uses the same
Deno graph tooling as `bundle-js`, but plugin HTTP imports use the plugin
domain allowlists rather than `allowed_base_urls`. Data access from Vega specs,
font downloads, and plugin imports are separate policies.

Startup plugins are loaded into each worker and execute in the order they are
configured.

::::{interface} python rust server
Per-request plugins run after startup plugins on an ephemeral worker for that
request.
::::

Plugin code should be ESM that registers Vega extensions. It should not rely on
browser globals such as `window` or `document` for server-side conversions.
Generated HTML can use plugins too; when plugins are present, VlConvert emits a
module script so plugin modules can be imported before `vegaEmbed()` runs.

HTML output has one extra CDN distinction. With `bundle=true`, plugin code is
embedded in the generated document. With `bundle=false`, URL-backed startup
plugins are imported from their original URLs by the browser so normal CDN
caching can apply. File and inline plugins are embedded in the generated HTML
even when Vega and Vega Embed are loaded from the CDN.
