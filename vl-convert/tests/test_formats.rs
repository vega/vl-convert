#![allow(deprecated)]

mod common;

use common::*;

#[test]
fn check_no_command() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = vl_convert_cmd()?;

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage: vl-convert"));
    Ok(())
}

const FORMAT_LOCALE_JSON: &str =
    r#"{"decimal":"d","thousands":"t","grouping":[3],"currency":["X",""]}"#;

const TIME_FORMAT_LOCALE_JSON: &str = r#"{
  "dateTime": "%x, %X",
  "date": "%m/%d/%Y",
  "time": "%H:%M:%S",
  "periods": ["AM", "PM"],
  "days": ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
  "shortDays": ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
  "months": ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
  "shortMonths": ["Jnx", "Fbx", "Mrx", "Apx", "May", "Jux", "Jlx", "Agx", "Spx", "Ocx", "Nvx", "Dcx"]
}"#;

const FORMAT_TEXT_SPEC: &str = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "width": 120,
  "height": 40,
  "data": {"values": [{"value": 1234.5}]},
  "mark": {"type": "text", "align": "left", "baseline": "top"},
  "encoding": {
    "x": {"value": 0},
    "y": {"value": 0},
    "text": {"field": "value", "type": "quantitative", "format": "$,.1f"}
  }
}"#;

const TIME_TEXT_SPEC: &str = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "width": 120,
  "height": 40,
  "data": {"values": [{"date": "2020-01-01T00:00:00"}]},
  "mark": {"type": "text", "align": "left", "baseline": "top"},
  "encoding": {
    "x": {"value": 0},
    "y": {"value": 0},
    "text": {"field": "date", "type": "temporal", "format": "%b"}
  }
}"#;

const SIMPLE_VL_SPEC: &str = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"values": [{"category": "A", "value": 1}]},
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "value", "type": "quantitative"}
  }
}"#;

fn write_temp_json(contents: &str) -> Result<NamedTempFile, Box<dyn std::error::Error>> {
    let mut file = tempfile::Builder::new().suffix(".json").tempfile()?;
    file.write_all(contents.as_bytes())?;
    Ok(file)
}

fn vl2svg_stdout(
    spec: &NamedTempFile,
    locale_flag: &str,
    locale_value: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = vl_convert_cmd()?;
    let out = cmd
        .arg("--vlc-config")
        .arg("disabled")
        .arg(locale_flag)
        .arg(locale_value)
        .arg("vl2svg")
        .arg("-i")
        .arg(spec.path())
        .output()?;

    assert!(
        out.status.success(),
        "vl2svg failed with status {:?}; stderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?)
}

#[test]
fn default_format_locale_accepts_inline_json_and_file() -> Result<(), Box<dyn std::error::Error>> {
    let spec = write_temp_json(FORMAT_TEXT_SPEC)?;

    let inline_svg = vl2svg_stdout(&spec, "--default-format-locale", FORMAT_LOCALE_JSON)?;
    assert!(
        inline_svg.contains("X1t234d5"),
        "inline default format locale was not applied; SVG:\n{inline_svg}"
    );

    let locale_file = write_temp_json(FORMAT_LOCALE_JSON)?;
    let file_svg = vl2svg_stdout(
        &spec,
        "--default-format-locale",
        locale_file.path().to_str().unwrap(),
    )?;
    assert!(
        file_svg.contains("X1t234d5"),
        "file default format locale was not applied; SVG:\n{file_svg}"
    );

    Ok(())
}

#[test]
fn default_time_locale_accepts_inline_json_and_file() -> Result<(), Box<dyn std::error::Error>> {
    let spec = write_temp_json(TIME_TEXT_SPEC)?;

    let inline_svg = vl2svg_stdout(
        &spec,
        "--default-time-format-locale",
        TIME_FORMAT_LOCALE_JSON,
    )?;
    assert!(
        inline_svg.contains("Jnx"),
        "inline default time-format locale was not applied; SVG:\n{inline_svg}"
    );

    let locale_file = write_temp_json(TIME_FORMAT_LOCALE_JSON)?;
    let file_svg = vl2svg_stdout(
        &spec,
        "--default-time-format-locale",
        locale_file.path().to_str().unwrap(),
    )?;
    assert!(
        file_svg.contains("Jnx"),
        "file default time-format locale was not applied; SVG:\n{file_svg}"
    );

    Ok(())
}

#[test]
fn vl2vg_render_overrides_set_top_level_properties() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let spec = write_temp_json(SIMPLE_VL_SPEC)?;
    let mut cmd = vl_convert_cmd()?;
    let output = cmd
        .arg("--vlc-config")
        .arg("disabled")
        .arg("vl2vg")
        .arg("-i")
        .arg(spec.path())
        .arg("--width")
        .arg("321")
        .arg("--height")
        .arg("123")
        .arg("--background")
        .arg("#abcdef")
        .output()?;

    assert!(
        output.status.success(),
        "vl2vg failed with status {:?}; stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let spec: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(spec["width"], 321);
    assert_eq!(spec["height"], 123);
    assert_eq!(spec["background"], "#abcdef");
    Ok(())
}

#[rustfmt::skip]
mod test_vl2vg {
    use std::fs;
    use crate::common::*;

    #[rstest]
    fn test(
        #[values(
            "v5_8",
            "v5_14",
            "v5_15",
            "v5_16",
            "v5_17",
            "v5_20",
            "v5_21",
            "v6_1",
            "v6_4",
        )]
        vl_version: &str,

        #[values("circle_binned", "seattle-weather", "stacked_bar_h")]
        name: &str,

        #[values(true, false)]
        pretty: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = if pretty {
            format!("{}_{}.pretty.vg.json", vl_version, name)
        } else {
            format!("{}_{}.vg.json", vl_version, name)
        };

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let mut cmd = cmd.arg("vl2vg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version);

        if pretty {
            cmd = cmd.arg("--pretty")
        }

        // Load expected
        match load_expected_vg_spec(name, vl_version, pretty) {
            Some(expected_str) => {
                cmd.assert().success();

                // Load written spec
                let output_str = fs::read_to_string(&output).unwrap();

                assert_eq!(expected_str, output_str)
            }
            None => {
                cmd.assert().failure();
            }
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2html_no_bundle {
    use std::fs;
    use crate::common::*;

    #[rstest]
    fn test(
        #[values(
            "5.8",
            "5.14",
            "5.15",
            "5.16",
            "5.17",
            "5.20",
            "5.21",
            "6.1",
            "6.4",
        )]
        vl_version: &str,

        #[values("circle_binned")]
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = format!("{}_{}.html", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2html")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version);
        cmd.assert().success();

        // Load written html
        let html_result = fs::read_to_string(&output).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(&format!("cdn.jsdelivr.net/npm/vega-lite@{vl_version}")));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega@6"));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega-embed@6"));

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2html_bundle {
    use std::fs;
    use crate::common::*;

    #[rstest]
    fn test(
        #[values(
            "5.8",
            "5.14",
            "5.15",
            "5.16",
            "5.17",
            "5.20",
            "5.21",
            "6.1",
            "6.4",
        )]
        vl_version: &str,

        #[values("circle_binned")]
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = format!("{}_{}.html", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2html")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--bundle");
        cmd.assert().success();

        // Load written html
        let html_result = fs::read_to_string(&output).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(vl_version));
        assert!(html_result.contains("<div id=\"vega-chart\">"));

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2svg {
    use std::fs;
    use crate::common::*;

    #[rstest]
    fn test(
        #[values(
            "v5_8",
        )]
        vl_version: &str,

        #[values("circle_binned", "stacked_bar_h")]
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = format!("{}_{}.svg", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2svg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir());

        // Load expected
        let expected_str = load_expected_svg(name, vl_version);
        cmd.assert().success();

        // Load written spec
        let output_str = fs::read_to_string(&output).unwrap();
        assert_eq!(expected_str, output_str);

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png {
    use crate::common::*;

    #[rstest(name, scale,
        case("circle_binned", 1.0),
        case("stacked_bar_h", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}.png", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, None).unwrap();


        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png_google_fonts {
    use crate::common::*;

    #[test]
    fn test() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_google_fonts.png", vl_version);

        let vl_path = vl_spec_path("google_fonts");
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--google-font").arg("Bangers")
            .arg("--google-font").arg("Lugrasimo")
            .arg("--scale").arg("2");

        cmd.assert().success();

        let expected_png = load_expected_png("google_fonts", vl_version, None).unwrap();
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for google_fonts.png with diff {}",
                diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png_theme_config {
    use crate::common::*;

    #[rstest(name, scale, theme,
    case("circle_binned", 1.0, "dark"),
    case("stacked_bar_h", 2.0, "vox")
    )]
    fn test(
        name: &str,
        scale: f32,
        theme: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}_theme.png", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        // Write config with background color to temporary file
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, r#"{{"background": "{}"}}"#, BACKGROUND_COLOR).unwrap();
        let config_path = config_file.path().to_str().unwrap();

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--theme").arg(theme)
            .arg("--config").arg(config_path)
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, Some(theme)).unwrap();

        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png_locale {
    use crate::common::*;

    #[rstest(name, scale,
        case("stocks_locale", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}.png", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        // Test with locale path
        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path.clone())
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--format-locale").arg(format_locale_path("it-IT"))
            .arg("--time-format-locale").arg(time_format_locale_path("it-IT"))
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, None).unwrap();

        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        // Test with locale name
        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--format-locale").arg("it-IT")
            .arg("--time-format-locale").arg("it-IT")
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, None).unwrap();

        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2jpeg {
    use crate::common::*;

    #[rstest(name, scale,
    case("circle_binned", 1.0),
    case("stacked_bar_h", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}.jpeg", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = vl_convert_cmd()?;
        let cmd = cmd.arg("vl2jpeg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--scale").arg(scale.to_string())
            .arg("--quality").arg("99");

        cmd.assert().success();

        // Load written spec
        let output_jpg = fs::read(&output).expect("Failed to read output image");
        assert_eq!(&output_jpg.as_slice()[..10], b"\xff\xd8\xff\xe0\x00\x10JFIF");

        Ok(())
    }
}

#[test]
fn test_ls_themes() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = vl_convert_cmd()?;
    let cmd = cmd.arg("ls-themes");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();
    assert!(output_str.contains("dark"));

    Ok(())
}

#[test]
fn test_ls_themes_with_vlc_config_custom_theme() -> Result<(), Box<dyn std::error::Error>> {
    // Write a JSONC config (with comments and trailing commas) that registers a custom theme.
    let mut config_file = NamedTempFile::with_suffix(".jsonc")?;
    writeln!(
        config_file,
        r##"{{
    // Custom theme for testing --vlc-config
    "themes": {{
        "my-custom-theme": {{
            "background": "#ff0000", // bright red background
            "view": {{ "stroke": "transparent" }},
        }}
    }},
}}"##
    )?;
    let config_path = config_file.path().to_str().unwrap();

    let mut cmd = vl_convert_cmd()?;
    let output = cmd
        .arg("--vlc-config")
        .arg(config_path)
        .arg("ls-themes")
        .output()?;

    assert!(output.status.success(), "ls-themes failed: {:?}", output);
    let output_str = String::from_utf8(output.stdout)?;

    // Built-in themes must still appear
    assert!(
        output_str.contains("dark"),
        "expected built-in theme 'dark' in output:\n{output_str}"
    );
    // Custom theme registered via vlc-config must appear
    assert!(
        output_str.contains("my-custom-theme"),
        "expected custom theme 'my-custom-theme' in output:\n{output_str}"
    );

    Ok(())
}

#[test]
fn test_cat_theme() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = vl_convert_cmd()?;
    let cmd = cmd.arg("cat-theme").arg("dark");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();

    // Check for known background color entry
    assert!(output_str.contains(r##""background": "#333"##));

    Ok(())
}
