---
title: Rust API Reference
path: api-reference
section: API Reference
order: 900
interfaces: [rust]
---

<!-- topic-body -->

# Rust API Reference

The package is named `vl-convert-rs` on crates.io and is imported as
`vl_convert_rs` in Rust code.

```rust
use vl_convert_rs::{PngOpts, VlcConfig, VlConverter, VlOpts};
```

Create one converter and reuse it for related work. `VlConverter::new()`
uses the default config. `VlConverter::with_config(...)` validates the config
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
- `ScenegraphMsgpackOutput`: `data`, `logs`, `google_fonts`.
- `VegaOutput`: `spec`, `logs`.

Applications that already use Tokio can await conversion methods directly. The
quick start shows the required runtime setup for a new application.

For item-by-item documentation, use the
[`vl-convert-rs` API on docs.rs](https://docs.rs/vl-convert-rs).
