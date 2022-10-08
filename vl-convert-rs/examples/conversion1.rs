use vl_convert_rs::{VlConverter, VlVersion};

#[tokio::main]
async fn main() {
    let mut converter = VlConverter::new();

    let vl_spec: serde_json::Value = serde_json::from_str(
        r#"
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"url": "data/movies.json"},
  "mark": "circle",
  "encoding": {
    "x": {
      "bin": {"maxbins": 10},
      "field": "IMDB Rating"
    },
    "y": {
      "bin": {"maxbins": 10},
      "field": "Rotten Tomatoes Rating"
    },
    "size": {"aggregate": "count"}
  }
}   "#,
    )
    .unwrap();

    let vega_spec = converter
        .vegalite_to_vega(vl_spec, VlVersion::v5_5, true)
        .await
        .expect("Failed to perform Vega-Lite to Vega conversion");

    println!("{}", vega_spec)
}
