---
title: Rust Converter API
path: advanced/rust-converter
section: Advanced
order: 440
interfaces: [rust]
---

<!-- topic-body -->

# Rust Converter Lifecycle

Create one `VlConverter` and reuse it for related conversion work.

```rust
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    num_workers: 4,
    ..Default::default()
});
```

`VlcConfig` controls worker count, data access, fonts, themes, plugins, and
V8 limits. Full Rust API documentation lives on
[docs.rs](https://docs.rs/vl-convert-rs/latest/vl_convert_rs/).
