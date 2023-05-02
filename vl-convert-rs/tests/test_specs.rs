use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use vl_convert_rs::text::register_font_directory;
use vl_convert_rs::{VlConverter, VlVersion};

use std::sync::Once;
use vl_convert_rs::converter::VlOpts;

static INIT: Once = Once::new();
const BACKGROUND_COLOR: &str = "#abc";

pub fn initialize() {
    INIT.call_once(|| {
        let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fonts_dir = root_path.join("tests").join("fonts");
        register_font_directory(fonts_dir.to_str().unwrap())
            .expect("Failed to register test font directory");
    });
}

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

fn load_expected_vg_spec(name: &str, vl_version: VlVersion) -> Option<serde_json::Value> {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(format!("{}.vg.json", name));
    if spec_path.exists() {
        let spec_str = fs::read_to_string(&spec_path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
        Some(serde_json::from_str(&spec_str).unwrap())
    } else {
        None
    }
}

fn make_expected_svg_path(name: &str, vl_version: VlVersion, theme: Option<&str>) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(if let Some(theme) = theme {
            format!("{}-{}.svg", name, theme)
        } else {
            format!("{}.svg", name)
        })
}

fn load_expected_svg(name: &str, vl_version: VlVersion, theme: Option<&str>) -> String {
    let spec_path = make_expected_svg_path(name, vl_version, theme);
    fs::read_to_string(&spec_path).unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path))
}

fn make_expected_png_path(name: &str, vl_version: VlVersion, theme: Option<&str>) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(if let Some(theme) = theme {
            format!("{}-{}.png", name, theme)
        } else {
            format!("{}.png", name)
        })
}

fn load_expected_png(name: &str, vl_version: VlVersion, theme: Option<&str>) -> Vec<u8> {
    let spec_path = make_expected_png_path(name, vl_version, theme);
    fs::read(&spec_path).unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path))
}

#[rustfmt::skip]
mod test_vegalite_to_vega {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::VlOpts;
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            VlVersion::v4_17,
            VlVersion::v5_2,
            VlVersion::v5_3,
            VlVersion::v5_4,
            VlVersion::v5_5,
            VlVersion::v5_6,
            VlVersion::v5_7,
            VlVersion::v5_8,
        )]
        vl_version: VlVersion,

        #[values("circle_binned", "seattle-weather", "stacked_bar_h")]
        name: &str,
    ) {
        initialize();

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        let vg_result = block_on(
            converter.vegalite_to_vega(vl_spec, VlOpts{vl_version, ..Default::default()}
            )
        );

        match load_expected_vg_spec(name, vl_version) {
            Some(expected_vg_spec) => {
                // Conversion is expected to succeed and match this
                let vg_result = vg_result.expect("Vega-Lite to Vega conversion failed");
                assert_eq!(vg_result, expected_vg_spec)
            }
            None => {
                println!("{:?}", vg_result);
                // Conversion is expected to fail
                assert!(vg_result.is_err())
            }
        }
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_svg {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::VlOpts;
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            "circle_binned",
            "stacked_bar_h",
            "bar_chart_trellis_compact",
            "line_with_log_scale",
            "numeric_font_weight"
        )]
        name: &str,
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Load expected SVG image
        let expected_svg = load_expected_svg(name, vl_version, None);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let svg = block_on(converter.vega_to_svg(vg_spec)).unwrap();
        assert_eq!(svg, expected_svg);

        // Convert directly to svg
        let svg = block_on(converter.vegalite_to_svg(vl_spec, VlOpts{vl_version, ..Default::default()})).unwrap();
        assert_eq!(svg, expected_svg);

        // // Write out reference image
        // let svg_path = make_expected_svg_path(name, vl_version, None);
        // std::fs::write(svg_path, svg).unwrap();
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_png {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::VlOpts;
    use vl_convert_rs::VlConverter;

    #[rstest(name, scale,
        case("circle_binned", 1.0),
        case("stacked_bar_h", 2.0),
        case("bar_chart_trellis_compact", 2.0),
        case("line_with_log_scale", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Load expected PNG image
        let expected_png_data = load_expected_png(name, vl_version, None);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec = block_on(
            converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})
        ).unwrap();

        let png_data = block_on(converter.vega_to_png(vg_spec, Some(scale))).unwrap();
        assert_eq!(png_data, expected_png_data);

        // Convert directly to png
        let png_data = block_on(
            converter.vegalite_to_png(vl_spec, VlOpts{vl_version, ..Default::default()}, Some(scale))
        ).unwrap();
        assert_eq!(png_data, expected_png_data);

        // // Write out reference image
        // let png_path = make_expected_png_path(name, vl_version, None);
        // std::fs::write(png_path, png_data).unwrap();
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_png_theme_config {
    use crate::*;
    use futures::executor::block_on;
    use serde_json::json;
    use vl_convert_rs::converter::VlOpts;
    use vl_convert_rs::VlConverter;

    #[rstest(name, scale, theme,
    case("circle_binned", 1.0, "dark"),
    case("stacked_bar_h", 2.0, "vox"),
    case("bar_chart_trellis_compact", 2.0, "excel"),
    case("line_with_log_scale", 2.0, "fivethirtyeight")
    )]
    fn test(
        name: &str,
        scale: f32,
        theme: &str,
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Load expected PNG image
        let expected_png_data = load_expected_png(name, vl_version, Some(theme));

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert directly to png with theme and config that overrides background color
        let png_data = block_on(
            converter.vegalite_to_png(
                vl_spec.clone(),
                VlOpts {
                    vl_version,
                    theme: Some(theme.to_string()),
                    config: Some(json!({"background": BACKGROUND_COLOR})),
                }, Some(scale)
            )
        ).unwrap();
        assert_eq!(png_data, expected_png_data);

        // Patch spec to put theme in `vl_spec.usermeta.embedOptions.theme` and don't pass theme
        // argument
        let mut usermeta_spec = vl_spec;
        let usermeta_spec_obj = usermeta_spec.as_object_mut().unwrap();
        usermeta_spec_obj.insert("usermeta".to_string(), json!({
            "embedOptions": {"theme": theme.to_string()}
        }));

        let png_data = block_on(
            converter.vegalite_to_png(
                usermeta_spec,
                VlOpts {
                    vl_version,
                    theme: None,
                    config: Some(json!({"background": BACKGROUND_COLOR})),
                }, Some(scale)
            )
        ).unwrap();
        assert_eq!(png_data, expected_png_data);

        // // Write out reference image
        // let png_path = make_expected_png_path(name, vl_version, Some(theme));
        // std::fs::write(png_path, png_data).unwrap();
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[tokio::test]
async fn test_font_with_quotes() {
    let vl_version = VlVersion::v5_8;

    // Load example Vega-Lite spec
    let name = "font_with_quotes";
    let vl_spec = load_vl_spec(name);

    // Create Vega-Lite Converter and perform conversion
    let mut converter = VlConverter::new();

    // Load expected PNG image
    let expected_png_data = load_expected_png(name, vl_version, None);

    let png_data = converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                ..Default::default()
            },
            Some(2.0),
        )
        .await
        .unwrap();

    assert_eq!(png_data, expected_png_data);

    // // Write out reference image
    // let png_path = make_expected_png_path(name, vl_version, None);
    // std::fs::write(png_path, png_data).unwrap();
}
