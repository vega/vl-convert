use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use rstest::rstest;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::str::FromStr; // Run programs
use std::sync::Once;
use tempfile::NamedTempFile;
use vl_convert_rs::VlVersion;
use dssim::{Dssim, DssimImage};

const BACKGROUND_COLOR: &str = "#abc";
static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let outdir = root_path.join("tests").join("output");
        fs::remove_dir_all(&outdir).ok();
        fs::create_dir_all(&outdir).unwrap();
    });
}

fn vl_spec_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join(format!("{}.vl.json", name));
    spec_path.to_str().unwrap().to_string()
}

fn load_expected_vg_spec(name: &str, vl_version: &str, pretty: bool) -> Option<String> {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
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

fn load_expected_svg(name: &str, vl_version: &str) -> String {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(format!("{}.svg", name));

    fs::read_to_string(spec_path).unwrap()
}

fn load_expected_png(name: &str, vl_version: &str, theme: Option<&str>) -> Option<DssimImage<f32>> {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let image_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(if let Some(theme) = theme {
            format!("{}-{}.png", name, theme)
        } else {
            format!("{}.png", name)
        });
    dssim::load_image(&Dssim::new(), image_path).ok()
}

fn output_path(filename: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("output")
        .join(filename)
        .to_str()
        .unwrap()
        .to_string()
}

fn test_font_dir() -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts_dir = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("fonts");
    fonts_dir.to_str().unwrap().to_string()
}

#[test]
fn check_no_command() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = Command::cargo_bin("vl-convert")?;

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage: vl-convert"));
    Ok(())
}

#[rustfmt::skip]
mod test_vl2vg {
    use std::fs;
    use std::process::Command;
    use crate::*;

    #[rstest]
    fn test(
        #[values(
            "4.17",
            "v5_6",
            "v5_7",
            "v5_8",
            "v5_9",
            "v5_10",
            "v5_11",
            "v5_12",
            "v5_13",
            "v5_14",
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

        let mut cmd = Command::cargo_bin("vl-convert")?;
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
mod test_vl2svg {
    use std::fs;
    use std::process::Command;
    use crate::*;

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

        let mut cmd = Command::cargo_bin("vl-convert")?;
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
    use std::process::Command;
    use crate::*;

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

        let mut cmd = Command::cargo_bin("vl-convert")?;
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
        let (diff, _) = attr.compare(&expected_png, &output_png);

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
mod test_vl2png_theme_config {
    use std::process::Command;
    use crate::*;

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

        let mut cmd = Command::cargo_bin("vl-convert")?;
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
        let (diff, _) = attr.compare(&expected_png, &output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[test]
fn test_ls_themes() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("vl-convert")?;
    let cmd = cmd.arg("ls-themes");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();
    assert!(output_str.contains("dark"));

    Ok(())
}

#[test]
fn test_cat_theme() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("vl-convert")?;
    let cmd = cmd.arg("cat-theme").arg("dark");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();

    // Check for known background color entry
    assert!(output_str.contains(r##""background": "#333"##));

    Ok(())
}
