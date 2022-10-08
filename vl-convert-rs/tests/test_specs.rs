use rstest::rstest;
use std::fs;
use std::path::Path;
use vl_convert_rs::VlVersion;

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
            VlVersion::v4_17,
            VlVersion::v5_0,
            VlVersion::v5_1,
            VlVersion::v5_2,
            VlVersion::v5_3,
            VlVersion::v5_4,
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
