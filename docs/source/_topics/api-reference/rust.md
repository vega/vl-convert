---
title: Rust API Reference
path: api-reference
section: API Reference
order: 900
interfaces: [rust]
---

<!-- topic-body -->

# Rust API Reference

The crate is named `vl-convert-rs` on crates.io and imported as `vl_convert_rs`.

```rust
use vl_convert_rs::{PngOpts, VlcConfig, VlConverter, VlOpts};
```

Create one converter and reuse it. `VlConverter::new()` uses the default configuration. `VlConverter::with_config()` validates a `VlcConfig` and returns a `Result`.

```rust
let converter = VlConverter::with_config(VlcConfig {
    num_workers: std::num::NonZeroU64::new(4).unwrap(),
    ..Default::default()
})?;
```

Conversion methods are `async` and return output structs rather than raw bytes, so callers can read Vega's diagnostic messages and Google Fonts usage. This API excerpt assumes `spec` contains a Vega-Lite specification. The complete Tokio application in {doc}`getting-started/quick-start` defines it:

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

Output structs:

- `PngOutput`, `JpegOutput`, and `PdfOutput`: `data`, `logs`, `google_fonts`.
- `SvgOutput`: `svg`, `logs`, `google_fonts`.
- `HtmlOutput`: `html`, `logs`, `google_fonts`.
- `ScenegraphOutput`: `scenegraph`, `logs`, `google_fonts`.
- `ScenegraphMsgpackOutput`: `data`, `logs`, `google_fonts`.
- `VegaOutput`: `spec`, `logs`.

{doc}`advanced/rust-converter` covers sharing the converter. Item-by-item documentation is on [docs.rs](https://docs.rs/vl-convert-rs).
