---
title: Rust Converter API
path: advanced/rust-converter
section: Advanced
order: 440
interfaces: [rust]
---

<!-- topic-body -->

# Rust Converter Lifecycle

Create one `VlConverter` and reuse it for related conversion work. A converter
owns a worker pool and its normalized `VlcConfig`; constructing one per request
adds avoidable startup cost.

```rust
use std::num::NonZeroU64;
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    num_workers: NonZeroU64::new(4).unwrap(),
    ..Default::default()
})?;
```

`VlcConfig` controls worker count, data access, fonts, themes, plugins, and
V8 limits. See {doc}`/rust/advanced/configuration` for the shared config shape.

Conversion methods are async. Call them from a Tokio runtime and share the
converter from application state when serving requests:

```rust
use std::sync::Arc;
use vl_convert_rs::{PngOpts, VlOpts};

let converter = Arc::new(converter);
let output = converter
    .vegalite_to_png(spec, VlOpts::default(), PngOpts::default())
    .await?;

std::fs::write("chart.png", output.data)?;
for log in output.logs {
    eprintln!("{}: {}", log.level, log.message);
}
```

Full Rust API documentation lives on
[docs.rs](https://docs.rs/vl-convert-rs/latest/vl_convert_rs/).
