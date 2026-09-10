---
title: Plugin Loading and Security
path: advanced/plugin-loading
section: Advanced
order: 480
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Plugin Loading and Security

Read {doc}`../guides/plugins` first for the plugin contract and a complete startup example. This page covers where plugin code can come from and how to control its network access.

## Choose a Loading Mode

Startup plugins are part of the converter configuration. VlConvert loads them in order when a worker starts and makes them available to every conversion that converter performs. Use startup plugins for stable extensions maintained with the application.

::::{interface} python rust server
Per-request plugins let a caller supply one plugin for one conversion. They are disabled by default and run in a separate temporary JavaScript runtime, which adds startup overhead and executes code chosen by the caller. Use them only when a trusted caller must choose plugin code dynamically.
::::

## Supported Plugin Sources

A plugin entry can be one of three forms:

- A `.js` or `.mjs` file path. VlConvert reads and bundles the file before loading it.
- An `http://` or `https://` URL. VlConvert fetches and bundles the module.
- Inline ESM source. Configuration files and library APIs accept source text. The `--vega-plugin` CLI flag does not, so a mistyped path is never treated as executable source.

The file-based examples use the `double-value.js` plugin from {doc}`../guides/plugins`.

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

```console
$ vl-convert --vega-plugin ./double-value.js \
>   vl2svg --input chart.vl.json --output chart.svg
```

Here, `chart.vl.json` is the plugin-dependent input from {doc}`../guides/plugins`.

Put inline source in a JSONC config file:

:::{dropdown} plugins.vlc.jsonc
:open:

```json
{
  "vega_plugins": [
    "export default function (vega) { vega.expressionFunction('answer', () => 42) }"
  ]
}
```
:::
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
Startup plugins come from global CLI options or the JSONC config file. Use the config file for inline source.

```console
$ vl-convert serve \
>   --vega-plugin ./double-value.js \
>   --port 3000
```
::::

## Control HTTP Imports

A plugin can import other ESM modules. HTTP imports are blocked unless their domain matches `plugin_import_domains`. The snippets in this section show the required plugin configuration. A complete chart must call `scaledPercent` in an expression before the plugin affects its result.

:::{dropdown} scale-plugin.js
:open:

```javascript
import { scaleLinear } from "https://esm.sh/d3-scale@4"

export default function registerScale(vega) {
  const scale = scaleLinear().domain([0, 1]).range([0, 100])
  vega.expressionFunction("scaledPercent", value => scale(value))
}
```
:::

::::{interface} python
```python
vlc.configure(
    vega_plugins=["./scale-plugin.js"],
    plugin_import_domains=["esm.sh"],
)
```
::::

::::{interface} cli
```console
$ vl-convert \
>   --vega-plugin ./scale-plugin.js \
>   --plugin-import-domains esm.sh \
>   vl2svg --input chart.vl.json --output chart.svg
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
```console
$ vl-convert serve \
>   --vega-plugin ./scale-plugin.js \
>   --plugin-import-domains esm.sh \
>   --port 3000
```
::::

`esm.sh` matches that host only. `*.jsdelivr.net` matches the named host and its subdomains. `*` allows any domain and should be reserved for trusted code. Redirect targets must also match the allowlist.

The domain of a URL plugin entry is added to the allowlist automatically, so the plugin can import from its own host. Imports from any other host still need an explicit entry. This allowlist is separate from `allowed_base_urls`, which controls data and image URLs.

## Bundle Multi-File Plugins

Bundle TypeScript and multi-file JavaScript before passing it to VlConvert. Prebundling keeps startup independent of package registries and produces one artifact that can be reviewed and deployed with the application.

```console
$ npm install --save-dev esbuild
$ npx esbuild src/acme-plugin.ts \
>   --bundle \
>   --format=esm \
>   --platform=browser \
>   --outfile=dist/acme-plugin.js
```

Register `dist/acme-plugin.js` as the startup plugin. The plugin must not rely on browser globals such as `window` or `document` during static conversion.

::::{interface} python rust server
## Enable Per-Request Plugins

Enable per-request plugins in the converter configuration, then pass one plugin as the conversion's `vega_plugin` value. The following examples reuse `double-value.js` and `chart.vl.json` from {doc}`../guides/plugins`.
::::

::::{interface} python
```python
from pathlib import Path

import vl_convert as vlc

vlc.configure(allow_per_request_plugins=True)

spec = Path("chart.vl.json").read_text(encoding="utf-8")
plugin_source = Path("double-value.js").read_text(encoding="utf-8")
svg = vlc.vegalite_to_svg(
    spec,
    vega_plugin=plugin_source,
)
Path("chart.svg").write_text(svg, encoding="utf-8")
```
::::

::::{interface} rust
```rust
use vl_convert_rs::{VlcConfig, VlConverter, VlOpts};

let converter = VlConverter::with_config(VlcConfig {
    allow_per_request_plugins: true,
    ..Default::default()
})?;
let spec = std::fs::read_to_string("chart.vl.json")?;
let plugin_source = std::fs::read_to_string("double-value.js")?;

let output = converter
    .vegalite_to_svg(
        spec,
        VlOpts {
            vega_plugin: Some(plugin_source),
            ..Default::default()
        },
        Default::default(),
    )
    .await?;
std::fs::write("chart.svg", output.svg)?;
```
::::

::::{interface} server
```console
$ vl-convert serve \
>   --port 3000 \
>   --allow-per-request-plugins \
>   --max-ephemeral-workers 2
```

Request bodies can now contain `vega_plugin`. If caller-supplied code needs HTTP imports, allow only the required domains with `--per-request-plugin-import-domains`. This allowlist is separate from the one for startup plugins.

Save this request, which includes the plugin and specification above, as `request.json`:

:::{dropdown} request.json
:open:

```{literalinclude} /_generated/requests/plugin-per-request-svg.json
:language: json
```
:::

```console
$ curl http://127.0.0.1:3000/vegalite/svg \
>   -H 'Content-Type: application/json' \
>   --data-binary @request.json \
>   --output chart.svg
```
::::

## Plugins in HTML Output

With `bundle=true`, the generated HTML contains its dependencies and the plugin code. With `bundle=false`, the browser loads Vega from a CDN, URL-backed startup plugins keep their original URLs so the browser loads them too, and file and inline plugins are embedded because the browser cannot read the converter's files. See {doc}`../guides/html-output`.
