---
title: Plugin Loading and Security
path: advanced/plugin-loading
section: Advanced
order: 475
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Plugin Loading and Security

Read {doc}`../guides/plugins` first for the plugin contract and a complete
startup example. This page covers where plugin code can come from and how to
control its network access.

## Choose a Loading Mode

Startup plugins are part of converter configuration. VlConvert loads them in
the configured order and makes them available to every conversion performed by
that converter. Use startup plugins for stable extensions maintained with the
application.

::::{interface} python rust server
Per-request plugins let a caller provide one plugin for one conversion. They
are disabled by default and use a separate temporary JavaScript runtime. This
adds startup overhead and executes code selected by the caller. Use this mode
only when a trusted caller must choose plugin code dynamically.
::::

## Supported Plugin Sources

VlConvert accepts three forms:

- A `.js` or `.mjs` file path. VlConvert reads and bundles the file before
  loading it.
- An `http://` or `https://` URL. VlConvert fetches and bundles the module.
- Inline ESM source. Configuration files and library APIs accept source text.
  The `--vega-plugin` CLI flag does not, which avoids treating a mistyped path
  as executable source.

::::{interface} python
```python
vlc.configure(
    vega_plugins=[
        "./double-value.js",
        "https://cdn.example.com/acme-plugin.js",
        "export default function (vega) { vega.expressionFunction('answer', () => 42) }",
    ]
)
```
::::

::::{interface} cli
Use a path or URL with `--vega-plugin`:

```bash
vl-convert --vega-plugin ./double-value.js \
  vl2svg --input chart.vl.json --output chart.svg
```

Put inline source in a JSONC config file:

```json
{
  "vega_plugins": [
    "export default function (vega) { vega.expressionFunction('answer', () => 42) }"
  ]
}
```
::::

::::{interface} rust
Each `vega_plugins` entry can be a path, URL, or source string:

```rust
let config = VlcConfig {
    vega_plugins: vec![
        "./double-value.js".to_string(),
        "https://cdn.example.com/acme-plugin.js".to_string(),
    ],
    ..Default::default()
};
```
::::

::::{interface} server
Startup plugins can come from global CLI options or the server's JSONC config
file. Use the config file for inline source.

```bash
vl-convert --vega-plugin ./double-value.js \
  serve --host 127.0.0.1 --port 3000
```
::::

## Control HTTP Imports

A plugin can import another ESM module. HTTP imports are blocked unless their
domains match `plugin_import_domains`.

```javascript
import { scaleLinear } from "https://esm.sh/d3-scale@4"

export default function registerScale(vega) {
  const scale = scaleLinear().domain([0, 1]).range([0, 100])
  vega.expressionFunction("scaledPercent", value => scale(value))
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

`esm.sh` matches that host only. `*.jsdelivr.net` matches the named host and
its subdomains. `*` allows any domain and should be reserved for trusted code.
Redirect destinations must also match the allowlist.

The domain of a URL plugin entry is allowed for that plugin, including relative
imports from the same host. Imports from any other host still need an explicit
allowlist entry. This plugin policy is separate from `allowed_base_urls`, which
controls data and image URLs used by specifications.

## Bundle Multi-File Plugins

Bundle TypeScript and multi-file JavaScript before passing it to VlConvert.
Prebundling makes startup independent of package registries and produces one
artifact that can be reviewed and deployed with the application.

```bash
npm install --save-dev esbuild
npx esbuild src/acme-plugin.ts \
  --bundle \
  --format=esm \
  --platform=browser \
  --outfile=dist/acme-plugin.js
```

Register `dist/acme-plugin.js` as the startup plugin. The plugin must not rely
on browser globals such as `window` or `document` during static conversion.

::::{interface} python rust server
## Enable Per-Request Plugins

Enable per-request plugins in converter configuration, then pass one plugin as
the conversion's `vega_plugin` value.
::::

::::{interface} python
```python
vlc.configure(allow_per_request_plugins=True)

svg = vlc.vegalite_to_svg(
    spec,
    vega_plugin="export default function (vega) { vega.expressionFunction('answer', () => 42) }",
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
            vega_plugin: Some(plugin_source.to_string()),
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

The request body can now contain `vega_plugin`. If caller-supplied code needs
HTTP imports, allow only the required domains with
`--per-request-plugin-import-domains`. This allowlist is separate from the one
for startup plugins.
::::

## Plugins in HTML Output

With `bundle=true`, generated HTML contains its dependencies and plugin code.
With `bundle=false`, the browser loads Vega dependencies from a content
delivery network. URL-backed startup plugins retain their original URLs so the
browser can load them. File and inline plugins are embedded because a browser
cannot access the converter's local files.
