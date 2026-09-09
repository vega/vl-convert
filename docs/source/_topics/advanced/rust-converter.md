---
title: Rust Converter Lifecycle
path: advanced/rust-converter
section: Advanced
order: 440
interfaces: [rust]
---

<!-- topic-body -->

# Rust Converter Lifecycle

Create one `VlConverter` per application and reuse it. The converter owns a lazily started pool of JavaScript workers. Creating a converter for every chart repeats that setup and prevents the pool from serving concurrent work.

```rust
use std::num::NonZeroU64;
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    num_workers: NonZeroU64::new(4).unwrap(),
    ..Default::default()
})?;

converter.warm_up()?;
```

`warm_up()` is optional. Call it when the application should pay the worker startup cost before accepting requests.

## Share the Converter

`VlConverter` is cheap to clone, and clones share the same worker pool. Store a clone in application state or hand clones to tasks. Each worker handles one conversion at a time, and additional conversions wait for a free worker. This lifecycle excerpt assumes the `spec` created in the complete {doc}`../getting-started/quick-start` application:

```rust
use vl_convert_rs::{PngOpts, VlOpts};

let task_converter = converter.clone();
let output = task_converter
    .vegalite_to_png(spec, VlOpts::default(), PngOpts::default())
    .await?;

std::fs::write("chart.png", output.data)?;

for entry in output.logs {
    eprintln!("{}: {}", entry.level, entry.message);
}
```

Conversion methods are `async` and run on Tokio or any other executor that can poll their futures. The {doc}`../getting-started/quick-start` includes a complete Tokio application.

## Configuration and Errors

`VlcConfig` controls worker count, data access, fonts, themes, plugins, and JavaScript resource limits. Build it before sharing the converter. See {doc}`configuration` and {doc}`memory-management`.

Methods return `Result` and keep the error context from compilation, data loading, fonts, plugins, and rendering. Successful outputs carry Vega's recoverable diagnostics in `logs`. See {doc}`../guides/logging`.

The complete crate API is on [docs.rs](https://docs.rs/vl-convert-rs).
