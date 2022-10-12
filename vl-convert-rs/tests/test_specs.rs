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

fn load_expected_svg(name: &str, vl_version: VlVersion) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(&format!("{:?}", vl_version))
        .join(format!("{}.svg", name));
    let svg_str =
        fs::read_to_string(&spec_path).unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
    svg_str
}

fn load_expected_png(name: &str, vl_version: VlVersion) -> Vec<u8> {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(&format!("{:?}", vl_version))
        .join(format!("{}.png", name));
    let png_data =
        fs::read(&spec_path).unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
    png_data
}

#[rustfmt::skip]
mod test_reference_specs {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            VlVersion::v4_17,
            VlVersion::v5_0,
            VlVersion::v5_1,
            VlVersion::v5_2,
            VlVersion::v5_3,
            VlVersion::v5_4,
            VlVersion::v5_5
        )]
        vl_version: VlVersion,

        #[values("circle_binned", "seattle-weather", "stacked_bar_h")]
        name: &str,

        #[values(false, true)]
        pretty: bool,
    ) {
        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        let vg_result = block_on(converter.vegalite_to_vega(vl_spec, vl_version, pretty));

        match load_expected_vg_spec(name, vl_version, pretty) {
            Some(expected_vg_spec) => {
                // Conversion is expected to succeed and match this
                let vg_result = vg_result.expect("Vega-Lite to Vega conversion failed");
                assert_eq!(vg_result, expected_vg_spec)
            }
            None => {
                // Conversion is expected to fail
                assert!(vg_result.is_err())
            }
        }
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

// mod test_reference_spec_svg {
//     use crate::*;
//     use futures::executor::block_on;
//     use vl_convert_rs::VlConverter;
//
//     #[rstest]
//     fn test(
//         #[values(
//         // VlVersion::v4_17,
//         // VlVersion::v5_0,
//         // VlVersion::v5_1,
//         // VlVersion::v5_2,
//         // VlVersion::v5_3,
//         // VlVersion::v5_4,
//         VlVersion::v5_5
//         )]
//         vl_version: VlVersion,
//
//         #[values(
//             "circle_binned",
//             // "seattle-weather"
//         )]
//         name: &str,
//     ) {
//         // Load example Vega-Lite spec
//         let vl_spec = load_vl_spec(name);
//
//         // Create Vega-Lite Converter and perform conversion
//         let mut converter = VlConverter::new();
//
//         let vg_result = block_on(converter.vegalite_to_vega(vl_spec, vl_version, false)).unwrap();
//         let vg_spec: serde_json::Value = serde_json::from_str(&vg_result).unwrap();
//
//         let svg = block_on(converter.vega_to_svg(vg_spec)).unwrap();
//
//         println!("{}", svg);
//     }
//
//     #[test]
//     fn test_marker() {} // Help IDE detect test module
// }

#[tokio::test]
async fn test_svg() {
    let name = "stacked_bar_h";
    let vl_version = VlVersion::v5_5;
    let vl_spec = load_vl_spec(name);
    let expected_svg = load_expected_svg(name, vl_version);
    let mut converter = VlConverter::new();

    // Convert to vega first
    let vg_spec: serde_json::Value = serde_json::from_str(
        &converter
            .vegalite_to_vega(vl_spec.clone(), vl_version, true)
            .await
            .unwrap(),
    )
    .unwrap();

    let svg = converter.vega_to_svg(vg_spec).await.unwrap();
    assert_eq!(svg, expected_svg);

    // Convert directly to svg
    let svg = converter
        .vegalite_to_svg(vl_spec, vl_version)
        .await
        .unwrap();
    assert_eq!(svg, expected_svg);

    // // Write out reference image
    // let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    // let svg_path = root_path
    //     .join("tests")
    //     .join("vl-specs")
    //     .join("expected")
    //     .join(format!("{:?}", vl_version))
    //     .join(format!("{}.svg", name));
    // std::fs::write(svg_path, svg).unwrap();
}

#[tokio::test]
async fn test_png() {
    let name = "stacked_bar_h";
    let vl_version = VlVersion::v5_5;
    let vl_spec = load_vl_spec(name);
    let expected_png_data = load_expected_png(name, vl_version);
    let mut converter = VlConverter::new();

    // Convert to vega first
    let vg_spec: serde_json::Value = serde_json::from_str(
        &converter
            .vegalite_to_vega(vl_spec.clone(), vl_version, true)
            .await
            .unwrap(),
    )
    .unwrap();

    let png_data = converter.vega_to_png(vg_spec, Some(2.0)).await.unwrap();
    assert_eq!(png_data, expected_png_data);

    // Convert directly to png
    let png_data = converter
        .vegalite_to_png(vl_spec, vl_version, Some(2.0))
        .await
        .unwrap();
    assert_eq!(png_data, expected_png_data);

    // // Write out reference image
    // let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    // let png_path = root_path
    //     .join("tests")
    //     .join("vl-specs")
    //     .join("expected")
    //     .join(format!("{:?}", vl_version))
    //     .join(format!("{}.png", name));
    // std::fs::write(png_path, png_data).unwrap();
}
