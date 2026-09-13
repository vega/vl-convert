## Overview

This crate provides a self-contained Rust library for converting
[Vega-Lite](https://vega.github.io/vega-lite/),
[Vega](https://vega.github.io/vega/), and SVG input. For Vega and Vega-Lite
charts, it produces SVG, PNG, JPEG, PDF, HTML, scenegraphs, font metadata, and
Vega Editor URLs. It also compiles Vega-Lite to Vega and converts existing SVG
to PNG, JPEG, or PDF. The conversions run the official Vega and Vega-Lite
JavaScript libraries in an embedded V8 runtime.

This crate is the conversion engine for the
[`vl-convert`](https://crates.io/crates/vl-convert) CLI and server and the
[`vl-convert-python`](https://pypi.org/project/vl-convert-python/) Python
package.

## Example

Use one [`VlConverter`] for Vega, Vega-Lite, and SVG conversions. This complete
example renders inline Vega-Lite data without network access:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
vl-convert-rs = "2"
```

```rust
use vl_convert_rs::{anyhow, PngOpts, VlConverter, VlOpts};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let spec = r#"{
      "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
      "data": {"values": [{"category": "A", "value": 3}, {"category": "B", "value": 7}]},
      "mark": "bar",
      "encoding": {
        "x": {"field": "category", "type": "nominal"},
        "y": {"field": "value", "type": "quantitative"}
      }
    }"#
    .to_string();

    let converter = VlConverter::new();
    let output = converter
        .vegalite_to_png(
            spec,
            VlOpts::default(),
            PngOpts {
                scale: Some(2.0),
                ..Default::default()
            },
        )
        .await?;
    std::fs::write("chart.png", output.data)?;
    Ok(())
}
```

## API guide

- [`VlConverter`] renders charts and manages reusable workers.
- [`VlcConfig`] controls fonts, access policies, plugins, and worker limits.
- [`VlOpts`] and [`VgOpts`] set per-conversion chart options.
- [`PngOpts`], [`SvgOpts`], [`JpegOpts`], and [`HtmlOpts`] control output.
- [`VlConverter::vegalite_fonts`] and [`VlConverter::vega_fonts`] inspect fonts.
- [`VlVersion`] and [`VL_VERSIONS`] select and list bundled Vega-Lite compilers.
- [`vegalite_to_url`] and [`vega_to_url`] create Vega Editor links without rendering.

Async conversion methods require Tokio. A converter starts its JavaScript workers
when needed and shares them across clones. SVG-only conversions use Tokio's
blocking pool without starting JavaScript workers.

Successful conversion outputs include the result and captured diagnostics.
Image and HTML outputs also report Google Fonts usage. See [`VlConverter`]
for shared error conditions and lifecycle behavior.
