use rstest::rstest;
use std::fs;
use std::path::Path;
use vl_convert_rs::{VlConverter, VlVersion};

fn load_vl_spec(name: &str) -> serde_json::Value {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("tests")
        .join("vl-specs")
        .join(format!("{}.vl.json", name));
    let spec_str =
        fs::read_to_string(&spec_path).unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
    serde_json::from_str(&spec_str)
        .unwrap_or_else(|_| panic!("Failed to parse {:?} as JSON", spec_path))
}

fn load_expected_vg_spec(name: &str, vl_version: VlVersion, pretty: bool) -> Option<String> {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(&format!("{:?}", vl_version))
        .join(if pretty {
            format!("{}.vg.pretty.json", name)
        } else {
            format!("{}.vg.json", name)
        });
    if spec_path.exists() {
        let spec_str = fs::read_to_string(&spec_path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
        Some(spec_str)
    } else {
        None
    }
}

mod test_reference_specs {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            // VlVersion::v4_17,
            // VlVersion::v5_0,
            // VlVersion::v5_1,
            // VlVersion::v5_2,
            // VlVersion::v5_3,
            // VlVersion::v5_4,
            VlVersion::v5_5
        )]
        vl_version: VlVersion,

        #[values("circle_binned", "seattle-weather")] name: &str,

        #[values(false, true)] pretty: bool,
    ) {
        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        let vg_result = block_on(converter.vegalite_to_vega(vl_spec, vl_version, pretty));

        match load_expected_vg_spec(name, vl_version, pretty) {
            Some(expected_vg_spec) => {
                // Conversion is expected to succeed and match this
                println!("expected_vg_spec:\n{}", expected_vg_spec);

                let vg_result = vg_result.expect("Vega-Lite to Vega conversion failed");
                assert_eq!(vg_result, expected_vg_spec)
            }
            None => {
                // Conversion is expected to fail
                assert!(vg_result.is_err())
            }
        }
        println!("{:?}", vl_version);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}


mod test_reference_spec_svg {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
        // VlVersion::v4_17,
        // VlVersion::v5_0,
        // VlVersion::v5_1,
        // VlVersion::v5_2,
        // VlVersion::v5_3,
        // VlVersion::v5_4,
        VlVersion::v5_5
        )]
        vl_version: VlVersion,

        #[values(
            "circle_binned",
            // "seattle-weather"
        )]
        name: &str,
    ) {
        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        let vg_result = block_on(converter.vegalite_to_vega(vl_spec, vl_version, false)).unwrap();
        let vg_spec: serde_json::Value = serde_json::from_str(&vg_result).unwrap();

        let svg = block_on(converter.vega_to_svg(vg_spec)).unwrap();

        println!("{}", svg);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[tokio::test]
async fn test_svg_font_metrics() {
    let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/next/data/barley.json"},
  "mark": "bar",
  "encoding": {
    "x": {"aggregate": "sum", "field": "yield"},
    "y": {"field": "variety"},
    "color": {"field": "site"}
  },
  "config": {
    "background": "aliceblue"
  }
}
    "#).unwrap();

//     let vl_spec: serde_json::Value = serde_json::from_str(r#"
// {
//   "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
//   "data": {"url": "http://data/barley.json"},
//   "mark": "bar",
//   "encoding": {
//     "x": {"aggregate": "sum", "field": "yield"},
//     "y": {"field": "variety"},
//     "color": {"field": "site"}
//   }
// }
//     "#).unwrap();

    let mut converter = VlConverter::new();
    let vg_spec: serde_json::Value = serde_json::from_str(
        &converter.vegalite_to_vega(vl_spec, VlVersion::v5_5, true).await.unwrap()
    ).unwrap();

    println!("vg_spec: {}", serde_json::to_string_pretty(&vg_spec).unwrap());
    let svg = converter.vega_to_svg(vg_spec).await.unwrap();
    println!("svg: {}", svg);

    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let svg_path = root_path
        .join("tests")
        .join("output")
        .join("stacked_bar_h.svg");
    std::fs::write(svg_path, svg).unwrap();
}


#[tokio::test]
async fn test_png() {
    let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/next/data/barley.json"},
  "mark": "bar",
  "encoding": {
    "x": {"aggregate": "sum", "field": "yield"},
    "y": {"field": "variety"},
    "color": {"field": "site"}
  },
  "config": {
    "background": "aliceblue"
  }
}
    "#).unwrap();

    let mut converter = VlConverter::new();
    let vg_spec: serde_json::Value = serde_json::from_str(
        &converter.vegalite_to_vega(vl_spec, VlVersion::v5_5, true).await.unwrap()
    ).unwrap();

    let svg_data = converter.vega_to_png(vg_spec, Some(2.0)).await.unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let png_path = root_path
        .join("tests")
        .join("output")
        .join("stacked_bar_h_out.png");
    std::fs::write(png_path, svg_data).unwrap();
}

