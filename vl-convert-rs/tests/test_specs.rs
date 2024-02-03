use dssim::{Dssim, DssimImage};
use rstest::rstest;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use vl_convert_rs::text::register_font_directory;
use vl_convert_rs::{VlConverter, VlVersion};

use serde_json::Value;
use std::sync::Once;
use vl_convert_rs::converter::{FormatLocale, TimeFormatLocale, VlOpts};

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

fn load_locale(
    format_name: &str,
    time_format_name: &str,
) -> (serde_json::Value, serde_json::Value) {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let locale_path = root_path.join("tests").join("locale");

    let format_path = locale_path
        .join("format")
        .join(format!("{format_name}.json"));
    let time_format_path = locale_path
        .join("time-format")
        .join(format!("{time_format_name}.json"));

    let format_str = fs::read_to_string(&format_path)
        .unwrap_or_else(|_| panic!("Failed to read {:?}", format_path));
    let time_format_str = fs::read_to_string(&time_format_path)
        .unwrap_or_else(|_| panic!("Failed to read {:?}", time_format_path));

    let format_value = serde_json::from_str(&format_str)
        .unwrap_or_else(|_| panic!("Failed to parse {:?} as JSON", format_str));
    let time_format_value = serde_json::from_str(&time_format_str)
        .unwrap_or_else(|_| panic!("Failed to parse {:?} as JSON", time_format_str));
    (format_value, time_format_value)
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

fn write_failed_vg(name: &str, vl_version: VlVersion, vg_spec: Option<Value>) {
    if let Some(vg_spec) = vg_spec {
        let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let failed_dir = root_path
            .join("tests")
            .join("vl-specs")
            .join("failed")
            .join(format!("{:?}", vl_version));

        fs::create_dir_all(failed_dir.clone()).unwrap();

        // Write standard
        let file_path = failed_dir.join(format!("{}.vg.json", name));
        let mut file = fs::File::create(file_path.clone()).unwrap();
        file.write_all(serde_json::to_string(&vg_spec).unwrap().as_bytes())
            .unwrap();

        // Write pretty
        let file_path = failed_dir.join(format!("{}.vg.pretty.json", name));
        let mut file = fs::File::create(file_path.clone()).unwrap();
        file.write_all(serde_json::to_string_pretty(&vg_spec).unwrap().as_bytes())
            .unwrap();
    }
}

fn check_vg(name: &str, vl_version: VlVersion, vg_spec: Option<Value>) {
    let expected = load_expected_vg_spec(name, vl_version);
    if vg_spec != expected {
        let path = write_failed_vg(name, vl_version, vg_spec);
        panic!(
            "Images don't match for {}.svg. Failed image written to {:?}",
            name, path
        )
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

fn make_expected_scenegraph_path(
    name: &str,
    vl_version: VlVersion,
    theme: Option<&str>,
) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(if let Some(theme) = theme {
            format!("{}-{}.sg.json", name, theme)
        } else {
            format!("{}.sg.json", name)
        })
}

fn load_expected_svg(name: &str, vl_version: VlVersion, theme: Option<&str>) -> Option<String> {
    let spec_path = make_expected_svg_path(name, vl_version, theme);
    fs::read_to_string(&spec_path).ok()
}

fn load_expected_scenegraph(
    name: &str,
    vl_version: VlVersion,
    theme: Option<&str>,
) -> Option<String> {
    let spec_path = make_expected_scenegraph_path(name, vl_version, theme);
    let Some(p) = fs::read_to_string(&spec_path).ok() else {
        return None;
    };
    Some(p)
}

fn write_failed_svg(name: &str, vl_version: VlVersion, theme: Option<&str>, img: &str) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let failed_dir = root_path
        .join("tests")
        .join("vl-specs")
        .join("failed")
        .join(format!("{:?}", vl_version));

    fs::create_dir_all(failed_dir.clone()).unwrap();

    let file_path = failed_dir.join(if let Some(theme) = theme {
        format!("{}-{}.svg", name, theme)
    } else {
        format!("{}.svg", name)
    });

    let mut file = fs::File::create(file_path.clone()).unwrap();
    file.write_all(img.as_bytes()).unwrap();
    return file_path;
}

fn write_failed_scenegraph(
    name: &str,
    vl_version: VlVersion,
    theme: Option<&str>,
    sg: &Value,
) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let failed_dir = root_path
        .join("tests")
        .join("vl-specs")
        .join("failed")
        .join(format!("{:?}", vl_version));

    fs::create_dir_all(failed_dir.clone()).unwrap();

    let file_path = failed_dir.join(if let Some(theme) = theme {
        format!("{}-{}.sg.json", name, theme)
    } else {
        format!("{}.sg.json", name)
    });

    let mut file = fs::File::create(file_path.clone()).unwrap();
    file.write_all(serde_json::to_string_pretty(sg).unwrap().as_bytes())
        .unwrap();
    return file_path;
}

fn check_svg(name: &str, vl_version: VlVersion, theme: Option<&str>, img: &str) {
    let expected = load_expected_svg(name, vl_version, theme);
    if Some(img.to_string()) != expected {
        let path = write_failed_svg(name, vl_version, None, img);
        panic!(
            "Images don't match for {}.svg. Failed image written to {:?}",
            name, path
        )
    }
}

fn check_scenegraph(name: &str, vl_version: VlVersion, theme: Option<&str>, sg: &Value) {
    let expected = load_expected_scenegraph(name, vl_version, theme);
    if let Some(expected) = &expected {
        let result_pretty = serde_json::to_string_pretty(sg).unwrap();
        if expected != &result_pretty {
            let path = write_failed_scenegraph(name, vl_version, theme, sg);
            println!(
                "Scenegraphs don't match for {}.sg.json. Failed image written to {:?}",
                name, path
            );
            assert_eq!(&result_pretty, expected)
        }
    }
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

fn load_expected_png_dssim(
    name: &str,
    vl_version: VlVersion,
    theme: Option<&str>,
) -> Option<DssimImage<f32>> {
    let spec_path = make_expected_png_path(name, vl_version, theme);
    dssim::load_image(&Dssim::new(), spec_path).ok()
}

fn to_dssim(img: &[u8]) -> DssimImage<f32> {
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    tmpfile.write_all(img).unwrap();
    dssim::load_image(&Dssim::new(), tmpfile.path()).unwrap()
}

fn write_failed_png(name: &str, vl_version: VlVersion, theme: Option<&str>, img: &[u8]) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let failed_dir = root_path
        .join("tests")
        .join("vl-specs")
        .join("failed")
        .join(format!("{:?}", vl_version));

    fs::create_dir_all(failed_dir.clone()).unwrap();

    let file_path = failed_dir.join(if let Some(theme) = theme {
        format!("{}-{}.png", name, theme)
    } else {
        format!("{}.png", name)
    });

    let mut file = fs::File::create(file_path.clone()).unwrap();
    file.write_all(img).unwrap();
    return file_path;
}

fn check_png(name: &str, vl_version: VlVersion, theme: Option<&str>, img: &[u8]) {
    let expected_dssim = load_expected_png_dssim(name, vl_version, theme);
    if let Some(expected_dssim) = expected_dssim {
        let img_dssim = to_dssim(img);

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_dssim, img_dssim);

        if diff > 0.00011 {
            println!("DSSIM diff {diff}");
            let path = write_failed_png(name, vl_version, None, img);
            panic!(
                "Images don't match for {}.png. Failed image written to {:?}",
                name, path
            )
        }
    } else {
        let path = write_failed_png(name, vl_version, None, img);
        panic!(
            "Baseline image does not exist for {}.png. Failed image written to {:?}",
            name, path
        )
    }
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
            VlVersion::v5_8,
            VlVersion::v5_9,
            VlVersion::v5_10,
            VlVersion::v5_11,
            VlVersion::v5_12,
            VlVersion::v5_13,
            VlVersion::v5_14,
            VlVersion::v5_15,
            VlVersion::v5_16,
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
        ).ok();

        check_vg(name, vl_version, vg_result);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_vegalite_to_html_no_bundle {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{Renderer, VlOpts};
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            VlVersion::v5_8,
            VlVersion::v5_9,
            VlVersion::v5_10,
            VlVersion::v5_11,
            VlVersion::v5_12,
            VlVersion::v5_13,
            VlVersion::v5_14,
            VlVersion::v5_15,
            VlVersion::v5_16,
        )]
        vl_version: VlVersion,

        #[values("circle_binned")]
        name: &str,
    ) {
        initialize();

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        let html_result = block_on(
            converter.vegalite_to_html(vl_spec, VlOpts{vl_version, ..Default::default()}, false, Renderer::Canvas)
        ).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(&format!("cdn.jsdelivr.net/npm/vega-lite@{}", vl_version.to_semver())));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega@5"));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega-embed@6"));
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_vegalite_to_html_bundle {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{Renderer, VlOpts};
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            VlVersion::v5_8,
            VlVersion::v5_9,
            VlVersion::v5_10,
            VlVersion::v5_11,
            VlVersion::v5_12,
            VlVersion::v5_13,
            VlVersion::v5_14,
            VlVersion::v5_15,
            VlVersion::v5_16,
        )]
        vl_version: VlVersion,

        #[values("circle_binned")]
        name: &str,
    ) {
        initialize();

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        let html_result = block_on(
            converter.vegalite_to_html(vl_spec, VlOpts{vl_version, ..Default::default()}, true, Renderer::Svg)
        ).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(vl_version.to_semver()));
        assert!(html_result.contains("Jeffrey Heer"));
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
            "circle_binned_base_url",
            "stacked_bar_h",
            "bar_chart_trellis_compact",
            "line_with_log_scale",
            "numeric_font_weight",
            "float_font_size",
            "no_text_in_font_metrics"
        )]
        name: &str,
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let svg = block_on(converter.vega_to_svg(vg_spec, Default::default())).unwrap();
        check_svg(name, vl_version, None, &svg);

        // Convert directly to svg
        let svg = block_on(converter.vegalite_to_svg(vl_spec, VlOpts{vl_version, ..Default::default()})).unwrap();
        check_svg(name, vl_version, None, &svg);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_svg_allowed_base_url {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{VgOpts, VlOpts};
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(#[values("circle_binned")] name: &str) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec = block_on(converter.vegalite_to_vega(
            vl_spec.clone(),
            VlOpts {
                vl_version,
                ..Default::default()
            },
        ))
        .unwrap();

        // Check with matching base URL
        let allowed_base_urls = Some(vec![
            "https://raw.githubusercontent.com/vega/vega-datasets".to_string()
        ]);
        let svg = block_on(converter.vega_to_svg(
            vg_spec.clone(),
            VgOpts {
                allowed_base_urls: allowed_base_urls.clone(),
                ..Default::default()
            },
        ))
        .unwrap();
        check_svg(name, vl_version, None, &svg);

        // Convert directly to svg
        let svg = block_on(converter.vegalite_to_svg(
            vl_spec.clone(),
            VlOpts {
                vl_version,
                allowed_base_urls,
                ..Default::default()
            },
        ))
        .unwrap();
        check_svg(name, vl_version, None, &svg);

        // Check for error with non-matching URL
        let allowed_base_urls = Some(vec!["https://some-other-base".to_string()]);

        let Err(result) = block_on(converter.vega_to_svg(
            vg_spec,
            VgOpts {
                allowed_base_urls: allowed_base_urls.clone(),
                ..Default::default()
            },
        )) else {
            panic!("Expected error")
        };
        assert!(result.to_string().contains("External data url not allowed"));

        let Err(result) = block_on(converter.vegalite_to_svg(
            vl_spec,
            VlOpts {
                vl_version,
                allowed_base_urls: allowed_base_urls.clone(),
                ..Default::default()
            },
        )) else {
            panic!("Expected error")
        };
        assert!(result.to_string().contains("External data url not allowed"));
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_scenegraph {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::VlOpts;
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(
        #[values(
            "no_text_in_font_metrics",
            "geoScale",
            "table_heatmap",
        )]
        name: &str,
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let sg = block_on(converter.vega_to_scenegraph(vg_spec, Default::default())).unwrap();
        check_scenegraph(name, vl_version, None, &sg);

        // Convert directly to svg
        let sg = block_on(converter.vegalite_to_scenegraph(vl_spec, VlOpts{vl_version, ..Default::default()})).unwrap();
        check_scenegraph(name, vl_version, None, &sg);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_png_no_theme {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::VlOpts;
    use vl_convert_rs::VlConverter;

    #[rstest(name, scale,
        case("circle_binned", 1.0),
        case("circle_binned_base_url", 1.0),
        case("stacked_bar_h", 2.0),
        case("bar_chart_trellis_compact", 2.0),
        case("line_with_log_scale", 2.0),
        case("remote_images", 1.0),
        case("maptile_background", 1.0),
        case("float_font_size", 1.0),
        case("no_text_in_font_metrics", 1.0),
        case("custom_projection", 1.0),
        case("long_legend_label", 1.0),
        case("quakes_initial_selection", 1.0),
        case("geoScale", 1.0),
        case("table_heatmap", 1.0),
    )]
    fn test(
        name: &str,
        scale: f32
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec = block_on(
            converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})
        ).unwrap();

        let png_data = block_on(converter.vega_to_png(vg_spec, Default::default(), Some(scale), None)).unwrap();
        check_png(name, vl_version, None, png_data.as_slice());

        // Convert directly to png
        let png_data = block_on(
            converter.vegalite_to_png(vl_spec, VlOpts{vl_version, ..Default::default()}, Some(scale), None)
        ).unwrap();
        check_png(name, vl_version, None, png_data.as_slice());
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
                    show_warnings: false,
                    allowed_base_urls: None,
                    format_locale: None,
                    time_format_locale: None,
                },
                Some(scale),
                None
            )
        ).unwrap();
        check_png(name, vl_version, Some(theme), png_data.as_slice());

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
                    show_warnings: false,
                    allowed_base_urls: None,
                    format_locale: None,
                    time_format_locale: None,
                },
                Some(scale),
                None
            )
        ).unwrap();
        check_png(name, vl_version, Some(theme), png_data.as_slice());
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

    let png_data = converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                ..Default::default()
            },
            Some(2.0),
            None,
        )
        .await
        .unwrap();

    check_png(name, vl_version, None, png_data.as_slice());
}

#[tokio::test]
async fn test_locale() {
    let vl_version = VlVersion::v5_8;

    // Load example Vega-Lite spec
    let name = "stocks_locale";
    let format_locale_name = "it-IT";
    let time_format_locale_name = "it-IT";
    let vl_spec = load_vl_spec(name);

    let (format_locale, time_format_locale) =
        load_locale(format_locale_name, time_format_locale_name);

    // Create Vega-Lite Converter and perform conversion
    let mut converter = VlConverter::new();

    // Convert with locale objects
    let png_data = converter
        .vegalite_to_png(
            vl_spec.clone(),
            VlOpts {
                vl_version,
                format_locale: Some(FormatLocale::Object(format_locale)),
                time_format_locale: Some(TimeFormatLocale::Object(time_format_locale)),
                ..Default::default()
            },
            Some(2.0),
            None,
        )
        .await
        .unwrap();

    check_png(name, vl_version, None, png_data.as_slice());

    // Convert with locale names
    let png_data = converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                format_locale: Some(FormatLocale::Name(format_locale_name.to_string())),
                time_format_locale: Some(TimeFormatLocale::Name(
                    time_format_locale_name.to_string(),
                )),
                ..Default::default()
            },
            Some(2.0),
            None,
        )
        .await
        .unwrap();

    check_png(name, vl_version, None, png_data.as_slice());
}
#[rustfmt::skip]
mod test_jpeg {
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
        "numeric_font_weight",
        "float_font_size",
        "no_text_in_font_metrics"
        )]
        name: &str,
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let mut converter = VlConverter::new();

        // Convert to vega first
        let vg_spec =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let jpeg_bytes = block_on(converter.vega_to_jpeg(vg_spec, Default::default(), None, None)).unwrap();

        // Check for JPEG prefix
        assert_eq!(&jpeg_bytes.as_slice()[..10], b"\xff\xd8\xff\xe0\x00\x10JFIF");

        // Convert directly to JPEG
        let jpeg_bytes = block_on(converter.vegalite_to_jpeg(vl_spec, VlOpts{vl_version, ..Default::default()}, None, None)).unwrap();
        assert_eq!(&jpeg_bytes.as_slice()[..10], b"\xff\xd8\xff\xe0\x00\x10JFIF");
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}
