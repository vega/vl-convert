mod test_utils;

use dssim::{Dssim, DssimImage};
use rstest::rstest;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use vl_convert_rs::{VlConverter, VlVersion};

use serde_json::Value;
use vl_convert_rs::converter::{
    FormatLocale, JpegOpts, PngOpts, SvgOpts, TimeFormatLocale, VlOpts,
};

use test_utils::{check_vg_png, initialize, load_vg_spec, to_dssim};

const BACKGROUND_COLOR: &str = "#abc";

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

const DEFAULT_THRESHOLD: f64 = 0.00011;

fn check_png(name: &str, vl_version: VlVersion, theme: Option<&str>, img: &[u8]) {
    check_png_with_threshold(name, vl_version, theme, img, DEFAULT_THRESHOLD);
}

fn check_png_with_threshold(
    name: &str,
    vl_version: VlVersion,
    theme: Option<&str>,
    img: &[u8],
    threshold: f64,
) {
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
                        if diff > threshold {
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
        ).ok().map(|output| output.spec);

        check_vg(name, vl_version, vg_result);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_vegalite_to_html_no_bundle {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{HtmlOpts, Renderer, VlOpts};
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

        let output = block_on(
            converter.vegalite_to_html(vl_spec, VlOpts{vl_version, ..Default::default()}, HtmlOpts { bundle: false, renderer: Renderer::Canvas })
        ).unwrap();

        // Check for expected patterns
        assert!(output.html.starts_with("<!DOCTYPE html>"));
        assert!(output.html.contains(&format!("cdn.jsdelivr.net/npm/vega-lite@{}", vl_version.to_semver())));
        assert!(output.html.contains("cdn.jsdelivr.net/npm/vega@6"));
        assert!(output.html.contains("cdn.jsdelivr.net/npm/vega-embed@6"));
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_vegalite_to_html_bundle {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{HtmlOpts, Renderer, VlOpts};
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

        let output = block_on(
            converter.vegalite_to_html(vl_spec, VlOpts{vl_version, ..Default::default()}, HtmlOpts { bundle: true, renderer: Renderer::Svg })
        ).unwrap();

        // Check for expected patterns
        assert!(output.html.starts_with("<!DOCTYPE html>"));
        assert!(output.html.contains(vl_version.to_semver()));
        assert!(output.html.contains("<div id=\"vega-chart\">"));
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
        let vg_output =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let output = block_on(converter.vega_to_svg(vg_output.spec, Default::default(), SvgOpts::default())).unwrap();
        check_svg(name, vl_version, None, &output.svg);

        // Convert directly to svg
        let output = block_on(converter.vegalite_to_svg(vl_spec, VlOpts{vl_version, ..Default::default()}, SvgOpts::default())).unwrap();
        check_svg(name, vl_version, None, &output.svg);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_svg_allowed_base_url {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{SvgOpts, VgOpts, VlOpts, VlcConfig};
    use vl_convert_rs::VlConverter;

    #[rstest]
    fn test(#[values("circle_binned")] name: &str) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create converter with matching base URL on the config
        let converter = VlConverter::with_config(VlcConfig {
            allowed_base_urls: Some(vec![
                "https://raw.githubusercontent.com/vega/vega-datasets".to_string()
            ]),
            ..Default::default()
        }).unwrap();

        // Convert to vega first
        let vg_output = block_on(converter.vegalite_to_vega(
            vl_spec.clone(),
            VlOpts {
                vl_version,
                ..Default::default()
            },
        ))
        .unwrap();

        // Check with matching base URL
        let output = block_on(converter.vega_to_svg(
            vg_output.spec.clone(),
            VgOpts::default(),
            SvgOpts::default(),
        ))
        .unwrap();
        check_svg(name, vl_version, None, &output.svg);

        // Convert directly to svg
        let output = block_on(converter.vegalite_to_svg(
            vl_spec.clone(),
            VlOpts {
                vl_version,
                ..Default::default()
            },
            SvgOpts::default(),
        ))
        .unwrap();
        check_svg(name, vl_version, None, &output.svg);

        // Check for error with non-matching URL
        let converter_blocked = VlConverter::with_config(VlcConfig {
            allowed_base_urls: Some(vec!["https://some-other-base".to_string()]),
            ..Default::default()
        }).unwrap();

        let Err(result) = block_on(converter_blocked.vega_to_svg(
            vg_output.spec,
            VgOpts::default(),
            SvgOpts::default(),
        )) else {
            panic!("Expected error")
        };
        assert!(result.to_string().contains("External data url not allowed"));

        let Err(result) = block_on(converter_blocked.vegalite_to_svg(
            vl_spec,
            VlOpts {
                vl_version,
                ..Default::default()
            },
            SvgOpts::default(),
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
        let vg_output =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let output = block_on(converter.vega_to_scenegraph(vg_output.spec, Default::default())).unwrap();
        check_scenegraph(name, vl_version, None, &output.scenegraph);

        // Convert directly to scenegraph
        let output = block_on(converter.vegalite_to_scenegraph(vl_spec, VlOpts{vl_version, ..Default::default()})).unwrap();
        check_scenegraph(name, vl_version, None, &output.scenegraph);
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

    #[rstest(name, scale, dssim_threshold,
        case("circle_binned", 1.0, DEFAULT_THRESHOLD),
        case("circle_binned_base_url", 1.0, DEFAULT_THRESHOLD),
        case("stacked_bar_h", 2.0, DEFAULT_THRESHOLD),
        case("stacked_bar_h2", 2.0, DEFAULT_THRESHOLD),
        case("bar_chart_trellis_compact", 2.0, DEFAULT_THRESHOLD),
        case("line_with_log_scale", 2.0, DEFAULT_THRESHOLD),
        case("remote_images", 1.0, DEFAULT_THRESHOLD),
        // Map tile specs use a relaxed threshold because OSM tiles change
        // over time as the tile provider updates their rendering style.
        case("maptile_background", 1.0, 0.02),
        case("maptile_background_2", 1.0, 0.02),
        case("float_font_size", 1.0, DEFAULT_THRESHOLD),
        case("no_text_in_font_metrics", 1.0, DEFAULT_THRESHOLD),
        case("custom_projection", 1.0, DEFAULT_THRESHOLD),
        case("long_legend_label", 1.0, DEFAULT_THRESHOLD),
        case("quakes_initial_selection", 1.0, DEFAULT_THRESHOLD),
        case("geoScale", 1.0, 0.02),
        case("table_heatmap", 1.0, DEFAULT_THRESHOLD),
        case("long_text_lable", 1.0, DEFAULT_THRESHOLD),
        case("gh_174", 1.0, DEFAULT_THRESHOLD),
        case("lookup_urls", 1.0, DEFAULT_THRESHOLD),
    )]
    fn test(
        name: &str,
        scale: f32,
        dssim_threshold: f64,
    ) {
        initialize();

        let vl_version = VlVersion::v5_8;

        // Load example Vega-Lite spec
        let vl_spec = load_vl_spec(name);

        // Create Vega-Lite Converter and perform conversion
        let converter = VlConverter::new();

        // Convert to vega first
        let vg_output = block_on(
            converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})
        ).unwrap();

        let output = block_on(converter.vega_to_png(vg_output.spec, Default::default(), PngOpts { scale: Some(scale), ppi: None })).unwrap();
        check_png_with_threshold(name, vl_version, None, output.data.as_slice(), dssim_threshold);

        // Convert directly to png
        let output = block_on(
            converter.vegalite_to_png(vl_spec, VlOpts{vl_version, ..Default::default()}, PngOpts { scale: Some(scale), ppi: None })
        ).unwrap();
        check_png_with_threshold(name, vl_version, None, output.data.as_slice(), dssim_threshold);
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

#[rustfmt::skip]
mod test_png_google_fonts {
    use crate::*;
    use futures::executor::block_on;
    use vl_convert_rs::converter::{GoogleFontRequest, VlcConfig, VlOpts};
    use vl_convert_rs::VlConverter;

    #[test]
    fn test() {
        initialize();

        let vl_version = VlVersion::v5_8;
        let vl_spec = load_vl_spec("google_fonts");
        let converter = VlConverter::with_config(VlcConfig {
            google_fonts: Some(vec![
                GoogleFontRequest { family: "Bangers".to_string(), variants: None },
                GoogleFontRequest { family: "Lugrasimo".to_string(), variants: None },
            ]),
            ..Default::default()
        }).unwrap();

        let vg_output = block_on(
            converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})
        ).unwrap();

        let output = block_on(converter.vega_to_png(vg_output.spec, Default::default(), PngOpts { scale: Some(2.0), ppi: None })).unwrap();
        check_png("google_fonts", vl_version, None, output.data.as_slice());

        let output = block_on(
            converter.vegalite_to_png(vl_spec, VlOpts{vl_version, ..Default::default()}, PngOpts { scale: Some(2.0), ppi: None })
        ).unwrap();
        check_png("google_fonts", vl_version, None, output.data.as_slice());
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
        let output = block_on(
            converter.vegalite_to_png(
                vl_spec.clone(),
                VlOpts {
                    vl_version,
                    theme: Some(theme.to_string()),
                    config: Some(json!({"background": BACKGROUND_COLOR})),
                    ..Default::default()
                },
                PngOpts { scale: Some(scale), ppi: None },
            )
        ).unwrap();
        check_png(name, vl_version, Some(theme), output.data.as_slice());

        // Patch spec to put theme in `vl_spec.usermeta.embedOptions.theme` and don't pass theme
        // argument
        let mut usermeta_spec = vl_spec;
        let usermeta_spec_obj = usermeta_spec.as_object_mut().unwrap();
        usermeta_spec_obj.insert("usermeta".to_string(), json!({
            "embedOptions": {"theme": theme.to_string()}
        }));

        let output = block_on(
            converter.vegalite_to_png(
                usermeta_spec,
                VlOpts {
                    vl_version,
                    config: Some(json!({"background": BACKGROUND_COLOR})),
                    ..Default::default()
                },
                PngOpts { scale: Some(scale), ppi: None },
            )
        ).unwrap();
        check_png(name, vl_version, Some(theme), output.data.as_slice());
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

    let output = converter
        .vegalite_to_png(
            vl_spec,
            VlOpts {
                vl_version,
                ..Default::default()
            },
            PngOpts {
                scale: Some(2.0),
                ppi: None,
            },
        )
        .await
        .unwrap();

    check_png(name, vl_version, None, output.data.as_slice());
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
    let output = converter
        .vegalite_to_png(
            vl_spec.clone(),
            VlOpts {
                vl_version,
                format_locale: Some(FormatLocale::Object(format_locale)),
                time_format_locale: Some(TimeFormatLocale::Object(time_format_locale)),
                ..Default::default()
            },
            PngOpts {
                scale: Some(2.0),
                ppi: None,
            },
        )
        .await
        .unwrap();

    check_png(name, vl_version, None, output.data.as_slice());

    // Convert with locale names
    let output = converter
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
            PngOpts {
                scale: Some(2.0),
                ppi: None,
            },
        )
        .await
        .unwrap();

    check_png(name, vl_version, None, output.data.as_slice());
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
        let vg_output =
            block_on(converter.vegalite_to_vega(vl_spec.clone(), VlOpts{vl_version, ..Default::default()})).unwrap();

        let output = block_on(converter.vega_to_jpeg(vg_output.spec, Default::default(), JpegOpts::default())).unwrap();

        // Check for JPEG prefix
        assert_eq!(&output.data.as_slice()[..10], b"\xff\xd8\xff\xe0\x00\x10JFIF");

        // Convert directly to JPEG
        let output = block_on(converter.vegalite_to_jpeg(vl_spec, VlOpts{vl_version, ..Default::default()}, JpegOpts::default())).unwrap();
        assert_eq!(&output.data.as_slice()[..10], b"\xff\xd8\xff\xe0\x00\x10JFIF");
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

        let output = block_on(
            converter.vega_to_png(vg_spec, Default::default(), PngOpts { scale: Some(scale), ppi: None })
        ).unwrap();

        check_vg_png(name, output.data.as_slice());
    }

    #[test]
    fn test_marker() {} // Help IDE detect test module
}

async fn check_svg_to_png_baseline(name: &str, svg: &str) {
    let converter = VlConverter::with_config(vl_convert_rs::converter::VlcConfig {
        auto_google_fonts: true,
        ..Default::default()
    })
    .unwrap();

    let output = converter
        .svg_to_png(
            svg,
            PngOpts {
                scale: Some(2.0),
                ppi: None,
            },
        )
        .await
        .unwrap();

    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected_path = root_path
        .join("tests")
        .join("svg-specs")
        .join("expected")
        .join(format!("{name}.png"));

    if expected_path.exists() {
        let expected_dssim = dssim::load_image(&Dssim::new(), &expected_path).unwrap();
        let actual_dssim = to_dssim(output.data.as_slice()).unwrap();
        let (diff, _) = Dssim::new().compare(&expected_dssim, actual_dssim);
        if diff > 0.00011 {
            let failed_dir = root_path.join("tests").join("svg-specs").join("failed");
            fs::create_dir_all(&failed_dir).unwrap();
            let failed_path = failed_dir.join(format!("{name}.png"));
            fs::write(&failed_path, &output.data).unwrap();
            panic!("DSSIM diff {diff} for {name}.png. Failed image written to {failed_path:?}");
        }
    } else {
        let failed_dir = root_path.join("tests").join("svg-specs").join("failed");
        fs::create_dir_all(&failed_dir).unwrap();
        let failed_path = failed_dir.join(format!("{name}.png"));
        fs::write(&failed_path, &output.data).unwrap();
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
