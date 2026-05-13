---
title: Rust API Reference
path: api-reference
section: API Reference
order: 900
interfaces: [rust]
---

<!-- topic-body -->

# Rust API

Use the Rust crate through `vl_convert_rs`.

```rust
use vl_convert_rs::{PngOpts, VlcConfig, VlConverter, VlOpts};
```

Create one converter and reuse it for related work. `VlConverter::new()`
uses the default config; `VlConverter::with_config(...)` validates the config
and returns a `Result`.

```rust
let converter = VlConverter::with_config(VlcConfig {
    num_workers: std::num::NonZeroU64::new(4).unwrap(),
    ..Default::default()
})?;
```

Conversion methods are async. They return output structs rather than raw bytes
so callers can inspect Vega logs and Google Fonts usage.

```rust
let output = converter
    .vegalite_to_png(
        spec,
        VlOpts::default(),
        PngOpts {
            scale: Some(2.0),
            ppi: None,
        },
    )
    .await?;

std::fs::write("chart.png", &output.data)?;
for entry in &output.logs {
    eprintln!("{}: {}", entry.level, entry.message);
}
```

Common output shapes:

- `PngOutput`, `JpegOutput`, and `PdfOutput`: `data`, `logs`, `google_fonts`.
- `SvgOutput`: `svg`, `logs`, `google_fonts`.
- `HtmlOutput`: `html`, `logs`, `google_fonts`.
- `ScenegraphOutput`: `scenegraph`, `logs`, `google_fonts`.
- `VegaOutput`: `spec`, `logs`.

Run these futures inside a Tokio runtime. Applications that already use Tokio
can await the methods directly; synchronous callers should create a runtime at
their own boundary.

Reference entry points:

- [`vl_convert_rs`](https://docs.rs/vl-convert-rs/latest/vl_convert_rs/)
- [`converter` module](https://docs.rs/vl-convert-rs/latest/vl_convert_rs/converter/)

The full generated API reference lives on
[docs.rs](https://docs.rs/vl-convert-rs/latest/vl_convert_rs/).
