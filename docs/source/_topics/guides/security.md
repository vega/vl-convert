---
title: Security and Data Access
path: guides/security
section: Guides
order: 270
interfaces: [python, cli, rust, server]
---

<!-- topic-body -->

# Security and Network Access

Configure data access explicitly when specs come from users or untrusted
systems. `allowed_base_urls` applies to Vega data loading; plugin imports and
Google Fonts downloads have separate controls.

::::{interface} python
```python
import vl_convert as vlc

vlc.configure(allowed_base_urls=["https://data.example.com/"])
```
::::

::::{interface} cli
```bash
vl-convert --allowed-base-urls https://data.example.com/ \
  vl2png \
  --input chart.vl.json --output chart.png
```
::::


::::{interface} rust
```rust
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    allowed_base_urls: vec!["https://data.example.com/".to_string()],
    ..Default::default()
})?;
```
::::


::::{interface} server
```bash
vl-convert serve \
  --allowed-base-urls=https://data.example.com/ \
  --opaque-errors=true
```

See {doc}`/server/authentication` and {doc}`/server/rate-limiting` for
server-specific controls. See {doc}`/server/guides/plugins` and
{doc}`/server/guides/fonts` for plugin and Google Fonts controls.
::::
