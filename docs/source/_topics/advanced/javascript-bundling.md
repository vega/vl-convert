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
let bundle = converter.javascript_bundle(None, None).await?;
```
::::


::::{interface} server
```bash
curl http://localhost:3000/bundling/javascript > vega-embed.js
```
::::
