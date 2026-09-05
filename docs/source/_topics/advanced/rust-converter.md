---
title: Rust Converter Lifecycle
path: advanced/rust-converter
section: Advanced
order: 440
interfaces: [rust]
---

<!-- topic-body -->

# Rust Converter Lifecycle

Create a `VlConverter` for an application boundary and reuse it. The converter
owns a lazily started pool of JavaScript workers. Constructing one for every
chart repeats setup and prevents the pool from serving concurrent work.

```rust
use std::num::NonZeroU64;
use vl_convert_rs::{VlcConfig, VlConverter};

let converter = VlConverter::with_config(VlcConfig {
    num_workers: NonZeroU64::new(4).unwrap(),
    ..Default::default()
})?;

converter.warm_up()?;
```

Warm-up is optional. Use it when the application should pay worker startup cost
before accepting requests.

## Share the Converter

`VlConverter::clone()` is inexpensive and shares the same underlying worker
pool. Store a clone in application state or pass clones to tasks. One worker
handles one conversion at a time, and extra conversions wait until a worker is
available.

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

Conversion methods are asynchronous and can run on Tokio or another executor
that can poll their futures. The quick start includes a complete Tokio
application.

## Configuration and Errors

`VlcConfig` controls worker count, data access, fonts, themes, plugins, and
JavaScript resource limits. Validate and construct it before sharing the
converter. See {doc}`configuration` for examples.

Methods return `Result` and preserve error context from compilation, data
loading, fonts, plugins, and rendering. Render outputs also carry recoverable
Vega diagnostic messages in `logs`.

The complete crate API is available on
[docs.rs](https://docs.rs/vl-convert-rs).
