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

fn load_vg_spec(name: &str) -> serde_json::Value {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("tests")
        .join("specs")
        .join(format!("{}.vg.json", name));
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
        write_failed_vg(name, vl_version, vg_spec);
        panic!("Vega conversions don't match for {}.vg.json", name)
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
    fs::read_to_string(spec_path).ok()
}

fn load_expected_scenegraph(
    name: &str,
    vl_version: VlVersion,
    theme: Option<&str>,
) -> Option<String> {
    let spec_path = make_expected_scenegraph_path(name, vl_version, theme);
    fs::read_to_string(spec_path).ok()
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
    file_path
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
    file_path
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

fn to_dssim(img: &[u8]) -> Result<DssimImage<f32>, Box<dyn std::error::Error>> {
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    tmpfile.write_all(img).unwrap();
    dssim::load_image(&Dssim::new(), tmpfile.path())
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
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
    file_path
}

fn check_png(name: &str, vl_version: VlVersion, theme: Option<&str>, img: &[u8]) {
    let expected_dssim = load_expected_png_dssim(name, vl_version, theme);
    if let Some(expected_dssim) = expected_dssim {
        match to_dssim(img) {
            Ok(img_dssim) => {
                let attr = Dssim::new();

                // Wrap the comparison in a panic-catching block to handle size mismatches
                let comparison_result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        attr.compare(&expected_dssim, img_dssim)
                    }));

                match comparison_result {
                    Ok((diff, _)) => {
                        if diff > 0.00011 {
                            println!("DSSIM diff {diff}");
                            let path = write_failed_png(name, vl_version, theme, img);
                            panic!(
                                "Images don't match for {}.png. Failed image written to {:?}",
                                name, path
                            )
                        }
                    }
                    Err(_) => {
                        let path = write_failed_png(name, vl_version, theme, img);
                        panic!(
                            "Image size mismatch for {}.png (cannot compare different sized images). Failed image written to {:?}",
                            name, path
                        )
                    }
                }
            }
            Err(e) => {
                let path = write_failed_png(name, vl_version, theme, img);
                panic!(
                    "Failed to process image for {}.png: {}. Failed image written to {:?}",
                    name, e, path
                )
            }
        }
    } else {
        let path = write_failed_png(name, vl_version, theme, img);
        panic!(
            "Baseline image does not exist for {}.png. Failed image written to {:?}",
            name, path
        )
    }
}

fn make_expected_vg_png_path(name: &str) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("specs")
        .join("expected")
        .join(format!("{}.png", name))
}

fn load_expected_vg_png_dssim(name: &str) -> Option<DssimImage<f32>> {
    let spec_path = make_expected_vg_png_path(name);
    dssim::load_image(&Dssim::new(), spec_path).ok()
}

fn write_failed_vg_png(name: &str, img: &[u8]) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let failed_dir = root_path.join("tests").join("specs").join("failed");

    fs::create_dir_all(failed_dir.clone()).unwrap();

    let file_path = failed_dir.join(format!("{}.png", name));

    let mut file = fs::File::create(file_path.clone()).unwrap();
    file.write_all(img).unwrap();
    file_path
}

fn check_vg_png(name: &str, img: &[u8]) {
    let expected_dssim = load_expected_vg_png_dssim(name);
    if let Some(expected_dssim) = expected_dssim {
        match to_dssim(img) {
            Ok(img_dssim) => {
                let attr = Dssim::new();

                // Wrap the comparison in a panic-catching block to handle size mismatches
                let comparison_result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        attr.compare(&expected_dssim, img_dssim)
                    }));

                match comparison_result {
                    Ok((diff, _)) => {
                        if diff > 0.00011 {
                            println!("DSSIM diff {diff}");
                            let path = write_failed_vg_png(name, img);
                            panic!(
                                "Images don't match for {}.png. Failed image written to {:?}",
                                name, path
                            )
                        }
                    }
                    Err(_) => {
                        let path = write_failed_vg_png(name, img);
                        panic!(
                            "Image size mismatch for {}.png (cannot compare different sized images). Failed image written to {:?}",
                            name, path
                        )
                    }
                }
            }
            Err(e) => {
                let path = write_failed_vg_png(name, img);
                panic!(
                    "Failed to process image for {}.png: {}. Failed image written to {:?}",
                    name, e, path
                )
            }
        }
    } else {
        let path = write_failed_vg_png(name, img);
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
            VlVersion::v5_14,
            VlVersion::v5_15,
            VlVersion::v5_16,
            VlVersion::v5_17,
            VlVersion::v5_20,
            VlVersion::v5_21,
            VlVersion::v6_1,
            VlVersion::v6_4,
        )]
        vl_version: VlVersion,

        #[values("circle_binned", "seattle-weather", "stacked_bar_h")]
        name: &str,
    ) {
        initialize();

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let converter = VlConverter::new();

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
            VlVersion::v5_14,
            VlVersion::v5_15,
            VlVersion::v5_16,
            VlVersion::v5_17,
            VlVersion::v5_20,
            VlVersion::v5_21,
            VlVersion::v6_1,
            VlVersion::v6_4,
        )]
        vl_version: VlVersion,

        #[values("circle_binned")]
        name: &str,
    ) {
        initialize();

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let converter = VlConverter::new();

        let html_result = block_on(
            converter.vegalite_to_html(vl_spec, VlOpts{vl_version, ..Default::default()}, false, false, true, Renderer::Canvas)
        ).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(&format!("cdn.jsdelivr.net/npm/vega-lite@{}", vl_version.to_semver())));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega@6"));
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
            VlVersion::v5_14,
            VlVersion::v5_15,
            VlVersion::v5_16,
            VlVersion::v5_17,
            VlVersion::v5_20,
            VlVersion::v5_21,
            VlVersion::v6_1,
            VlVersion::v6_4,
        )]
        vl_version: VlVersion,

        #[values("circle_binned")]
        name: &str,
    ) {
        initialize();

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let converter = VlConverter::new();

        let html_result = block_on(
            converter.vegalite_to_html(vl_spec, VlOpts{vl_version, ..Default::default()}, true, false, true, Renderer::Svg)
        ).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(vl_version.to_semver()));
        assert!(html_result.contains("<div id=\"vega-chart\">"));
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
        let converter = VlConverter::new();

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
        let converter = VlConverter::new();

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
        let converter = VlConverter::new();

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
        case("stacked_bar_h2", 2.0),
        case("bar_chart_trellis_compact", 2.0),
        case("line_with_log_scale", 2.0),
        case("remote_images", 1.0),
        case("maptile_background", 1.0),
        case("maptile_background_2", 1.0),
        case("float_font_size", 1.0),
        case("no_text_in_font_metrics", 1.0),
        case("custom_projection", 1.0),
        case("long_legend_label", 1.0),
        case("quakes_initial_selection", 1.0),
        case("geoScale", 1.0),
        case("table_heatmap", 1.0),
        case("long_text_lable", 1.0),
        case("gh_174", 1.0),
        case("lookup_urls", 1.0),
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
        let converter = VlConverter::new();

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
mod test_png_google_fonts {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{GoogleFontRequest, VlConverterConfig, VlOpts};
    use vl_convert_rs::VlConverter;

    #[test]
    fn test() {
        initialize();

        let vl_version = VlVersion::v5_8;
        let vl_spec = load_vl_spec("google_fonts");
        let converter = VlConverter::with_config(VlConverterConfig {
            google_fonts: Some(vec![
                GoogleFontRequest { family: "Bangers".to_string(), variants: None },
                GoogleFontRequest { family: "Lugrasimo".to_string(), variants: None },
            ]),
            ..Default::default()
        }).unwrap();

        let vg_spec = block_on(
            converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})
        ).unwrap();

        let png_data = block_on(converter.vega_to_png(vg_spec, Default::default(), Some(2.0), None)).unwrap();
        check_png("google_fonts", vl_version, None, png_data.as_slice());

        let png_data = block_on(
            converter.vegalite_to_png(vl_spec, VlOpts{vl_version, ..Default::default()}, Some(2.0), None)
        ).unwrap();
        check_png("google_fonts", vl_version, None, png_data.as_slice());
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
        let converter = VlConverter::new();

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
                    google_fonts: None,
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
                    google_fonts: None,
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
    let converter = VlConverter::new();

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
    let converter = VlConverter::new();

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
        let converter = VlConverter::new();

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

#[rustfmt::skip]
mod test_vega_label_transform {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::VlConverter;

    #[rstest(name, scale,
        case("label_transform_test", 1.0),
        case("label_scatter_plot", 1.0),
        case("label_movies_scatter", 1.0),
    )]
    fn test(name: &str, scale: f32) {
        initialize();

        let vg_spec = load_vg_spec(name);
        let converter = VlConverter::new();

        let png_data = block_on(
            converter.vega_to_png(vg_spec, Default::default(), Some(scale), None)
        ).unwrap();

        check_vg_png(name, png_data.as_slice());
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

async fn check_svg_to_png_baseline(name: &str, svg: &str) {
    let converter = VlConverter::with_config(vl_convert_rs::converter::VlConverterConfig {
        auto_google_fonts: true,
        ..Default::default()
    })
    .unwrap();

    let png_data = converter.svg_to_png(svg, 2.0, None).await.unwrap();

    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected_path = root_path
        .join("tests")
        .join("svg-specs")
        .join("expected")
        .join(format!("{name}.png"));

    if expected_path.exists() {
        let expected_dssim = dssim::load_image(&Dssim::new(), &expected_path).unwrap();
        let actual_dssim = to_dssim(png_data.as_slice()).unwrap();
        let (diff, _) = Dssim::new().compare(&expected_dssim, actual_dssim);
        if diff > 0.00011 {
            let failed_dir = root_path.join("tests").join("svg-specs").join("failed");
            fs::create_dir_all(&failed_dir).unwrap();
            let failed_path = failed_dir.join(format!("{name}.png"));
            fs::write(&failed_path, &png_data).unwrap();
            panic!("DSSIM diff {diff} for {name}.png. Failed image written to {failed_path:?}");
        }
    } else {
        let failed_dir = root_path.join("tests").join("svg-specs").join("failed");
        fs::create_dir_all(&failed_dir).unwrap();
        let failed_path = failed_dir.join(format!("{name}.png"));
        fs::write(&failed_path, &png_data).unwrap();
        panic!(
            "Baseline image does not exist for {name}.png. Failed image written to {failed_path:?}"
        );
    }
}

#[tokio::test]
async fn test_svg_to_png_auto_google_fonts_kalam() {
    initialize();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="250" height="60">
  <rect width="250" height="60" fill="#f0f0f0"/>
  <text x="10" y="40" font-family="Kalam" font-size="28" fill="#333">Hello Kalam</text>
</svg>"##;
    check_svg_to_png_baseline("svg_auto_google_fonts", svg).await;
}

#[tokio::test]
async fn test_svg_to_png_auto_google_fonts_pacifico() {
    initialize();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="60">
  <rect width="300" height="60" fill="#f0f0f0"/>
  <text x="10" y="42" font-family="Pacifico" font-size="30" fill="#333">Hello Pacifico</text>
</svg>"##;
    check_svg_to_png_baseline("svg_auto_google_fonts_pacifico", svg).await;
}

mod test_heap_limit {
    use vl_convert_rs::converter::{VgOpts, VlConverterConfig};
    use vl_convert_rs::VlConverter;

    /// Verify that exceeding the V8 heap limit returns a specific error
    /// rather than aborting the process, that the worker recovers and can
    /// process a subsequent conversion, and that memory stats are available
    /// before and after the OOM.
    #[tokio::test]
    async fn test_heap_limit_exceeded_and_recovery() {
        let converter = VlConverter::with_config(VlConverterConfig {
            max_worker_heap_size_mb: 256,
            ..Default::default()
        })
        .expect("Failed to create converter with small heap");

        // Check heap stats before OOM (also verifies pool auto-spawn)
        let stats_before = converter
            .get_worker_memory_usage()
            .await
            .expect("get_worker_memory_usage should succeed before OOM");
        assert_eq!(stats_before.len(), 1, "should have 1 worker");

        // Trigger OOM with a spec that exceeds the 256 MB heap limit
        let big_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 10,
            "height": 10,
            "data": [{
                "name": "big",
                "transform": [
                    { "type": "sequence", "start": 0, "stop": 50000000, "as": "x" }
                ]
            }],
            "marks": []
        });

        let result = converter.vega_to_svg(big_spec, VgOpts::default()).await;
        let err = result.expect_err("Expected heap limit error, got Ok");
        let msg = err.to_string();
        assert!(
            msg.contains("V8 heap limit exceeded"),
            "Error should mention heap limit, got: {msg}"
        );

        // Worker should still be responsive after OOM
        let stats_after = converter
            .get_worker_memory_usage()
            .await
            .expect("get_worker_memory_usage should succeed after OOM");
        assert_eq!(stats_after.len(), 1, "should still have 1 worker");

        // A normal conversion should succeed, proving recovery
        let small_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 10,
            "height": 10,
            "marks": []
        });
        let result = converter.vega_to_svg(small_spec, VgOpts::default()).await;
        assert!(
            result.is_ok(),
            "Conversion should succeed after recovery, got: {:?}",
            result.err()
        );
    }

    /// Verify that the heap limit is properly restored after recovery so
    /// the callback fires again on a second OOM.
    #[tokio::test]
    async fn test_heap_limit_restored_after_recovery() {
        let converter = VlConverter::with_config(VlConverterConfig {
            max_worker_heap_size_mb: 256,
            ..Default::default()
        })
        .expect("Failed to create converter with small heap");

        let big_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 10,
            "height": 10,
            "data": [{
                "name": "big",
                "transform": [
                    { "type": "sequence", "start": 0, "stop": 50000000, "as": "x" }
                ]
            }],
            "marks": []
        });

        // First OOM
        let result = converter
            .vega_to_svg(big_spec.clone(), VgOpts::default())
            .await;
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("V8 heap limit exceeded"), "First OOM: {msg}");

        // Recovery
        let small_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 10,
            "height": 10,
            "marks": []
        });
        assert!(converter
            .vega_to_svg(small_spec, VgOpts::default())
            .await
            .is_ok());

        // Second OOM — proves the limit was restored, not stuck at 2×
        let result = converter.vega_to_svg(big_spec, VgOpts::default()).await;
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("V8 heap limit exceeded"),
            "Second OOM should also be caught: {msg}"
        );
    }

    /// Verify that max_worker_heap_size_mb=0 (no limit) works: no callback
    /// is registered and a normal conversion succeeds.
    #[tokio::test]
    async fn test_no_heap_limit() {
        let converter = VlConverter::with_config(VlConverterConfig {
            max_worker_heap_size_mb: 0,
            ..Default::default()
        })
        .expect("max_worker_heap_size_mb=0 should be valid");

        let spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 10,
            "height": 10,
            "marks": []
        });

        let result = converter.vega_to_svg(spec, VgOpts::default()).await;
        assert!(
            result.is_ok(),
            "Conversion with no heap limit should succeed: {:?}",
            result.err()
        );
    }

    /// Verify that max_worker_heap_size_mb below the minimum is rejected
    /// at config time, not deferred to first use.
    #[test]
    fn test_min_heap_size_validation() {
        let result = VlConverter::with_config(VlConverterConfig {
            max_worker_heap_size_mb: 1,
            ..Default::default()
        });
        let err = result
            .err()
            .expect("max_worker_heap_size_mb=1 should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("too small for V8 to initialize"),
            "Should mention V8 initialization, got: {msg}"
        );
    }

    /// Smoke test that gc_after_conversion=true doesn't crash.
    #[tokio::test]
    async fn test_gc_after_conversion() {
        let converter = VlConverter::with_config(VlConverterConfig {
            gc_after_conversion: true,
            ..Default::default()
        })
        .expect("gc_after_conversion config should be valid");

        let spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 10,
            "height": 10,
            "marks": []
        });

        let result = converter.vega_to_svg(spec, VgOpts::default()).await;
        assert!(
            result.is_ok(),
            "Conversion with gc_after_conversion should succeed: {:?}",
            result.err()
        );
    }
}

mod test_plugin_custom_scheme_png {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{VgOpts, VlConverterConfig};
    use vl_convert_rs::VlConverter;

    #[test]
    fn test() {
        initialize();

        let plugin_source =
            "export default function(vega) { vega.scheme('testscheme', ['#ff0000', '#00ff00', '#0000ff']); }";

        let converter = VlConverter::with_config(VlConverterConfig {
            vega_plugins: Some(vec![plugin_source.to_string()]),
            ..Default::default()
        })
        .unwrap();

        let vg_spec = load_vg_spec("plugin_custom_scheme");
        let png_data = block_on(converter.vega_to_png(
            vg_spec,
            VgOpts::default(),
            Some(2.0),
            None,
        ))
        .unwrap();

        check_vg_png("plugin_custom_scheme", png_data.as_slice());
    }
}
