use vl_convert_rs::{anyhow, PngOpts, VlConverter, VlOpts};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let vl_spec = r#"{
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
            vl_spec,
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
