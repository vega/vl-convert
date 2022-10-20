use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use rstest::rstest;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str::FromStr; // Run programs
use std::sync::Once;
use vl_convert_rs::VlVersion;

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

fn load_expected_svg(name: &str, vl_version: &str) -> String {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(&format!("{:?}", vl_version))
        .join(format!("{}.svg", name));

    fs::read_to_string(&spec_path).unwrap()
}

fn load_expected_png(name: &str, vl_version: &str) -> Vec<u8> {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(&format!("{:?}", vl_version))
        .join(format!("{}.png", name));
    let png_data =
        fs::read(&spec_path).unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
    png_data
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
            "5_0",
            "v5.1",
            "v5_2",
            "v5_3",
            "v5_4",
            "v5_5",
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
            "v5_5",
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
    use std::fs;
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

        let vl_version = "5_5";
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

        // Load expected
        let expected_str = load_expected_png(name, vl_version);
        cmd.assert().success();

        // Load written spec
        let output_str = fs::read(&output).unwrap();
        assert_eq!(expected_str, output_str);

        Ok(())
    }
}
