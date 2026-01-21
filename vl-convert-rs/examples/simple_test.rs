use futures::executor::block_on;

fn main() {
    println!("Creating converter...");
    let mut converter = vl_convert_rs::VlConverter::new();

    // First, let's test with a simple script that doesn't use our custom ops
    // by checking the environment
    let vl_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "mark": "point",
        "data": {"values": [{"a": 1}]},
        "encoding": {"x": {"field": "a", "type": "quantitative"}}
    });

    println!("Running conversion...");

    match block_on(converter.vegalite_to_vega(
        vl_spec,
        vl_convert_rs::converter::VlOpts {
            vl_version: vl_convert_rs::VlVersion::v5_8,
            ..Default::default()
        },
    )) {
        Ok(result) => println!("Success:\n{}", serde_json::to_string_pretty(&result).unwrap()),
        Err(e) => println!("Error: {}", e),
    }
}
