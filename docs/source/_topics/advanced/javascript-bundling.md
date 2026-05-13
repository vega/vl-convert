---
title: JavaScript Bundling
path: advanced/javascript-bundling
section: Advanced
order: 470
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# JavaScript Bundling

The bundling APIs return the Vega Embed JavaScript bundle used by HTML output
and integrations.

::::{interface} python
```python
bundle = vlc.javascript_bundle()
bundle_with_snippet = vlc.javascript_bundle("console.log(vegaEmbed)")
```
::::

::::{interface} cli
```bash
vl-convert bundle-js --output vega-embed.js
vl-convert bundle-js --snippet snippet.js --output bundled-snippet.js
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{VlConverter, VlVersion};

let converter = VlConverter::new();
let bundle = converter.get_vegaembed_bundle(VlVersion::default()).await?;
let bundle_with_snippet = converter
    .bundle_vega_snippet("console.log(vegaEmbed)", VlVersion::default())
    .await?;
```
::::


::::{interface} server
```bash
curl http://localhost:3000/bundling/bundle > vega-embed.js

curl -X POST http://localhost:3000/bundling/bundle-snippet \
  -H 'Content-Type: application/json' \
  --data '{"snippet": "console.log(vegaEmbed)", "vl_version": "6.4"}' \
  > bundled-snippet.js
```
::::
