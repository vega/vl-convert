#![allow(dead_code, unused_imports)]

pub use assert_cmd::prelude::*;
pub use dssim::{Dssim, DssimImage};
pub use predicates::prelude::*;
pub use rstest::rstest;
pub use std::fs;
pub use std::io::Write;
pub use std::path::Path;
pub use std::process::Command;
pub use std::str::FromStr;
pub use std::sync::Once;
pub use tempfile::NamedTempFile;
pub use vl_convert_rs::VlVersion;

pub const BACKGROUND_COLOR: &str = "#abc";
pub static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let outdir = root_path.join("tests").join("output");
        fs::remove_dir_all(&outdir).ok();
        fs::create_dir_all(&outdir).unwrap();
    });
}

/// Build an `assert_cmd::Command` for the vl-convert binary. Centralized
/// so future test-wide CLI flags can be added in one place.
pub fn vl_convert_cmd() -> Result<Command, Box<dyn std::error::Error>> {
    Command::cargo_bin("vl-convert").map_err(Into::into)
}

pub fn vg_spec_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("specs")
        .join(format!("{}.vg.json", name))
        .to_str()
        .unwrap()
        .to_string()
}

pub fn vl_spec_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join(format!("{}.vl.json", name));
    spec_path.to_str().unwrap().to_string()
}

pub fn format_locale_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("locale")
        .join("format")
        .join(format!("{}.json", name));
    spec_path.to_str().unwrap().to_string()
}

pub fn time_format_locale_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("locale")
        .join("time-format")
        .join(format!("{}.json", name));
    spec_path.to_str().unwrap().to_string()
}

pub fn load_expected_vg_spec(name: &str, vl_version: &str, pretty: bool) -> Option<String> {
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

pub fn load_expected_svg(name: &str, vl_version: &str) -> String {
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

pub fn load_expected_png(
    name: &str,
    vl_version: &str,
    theme: Option<&str>,
) -> Option<DssimImage<f32>> {
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

pub fn output_path(filename: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("output")
        .join(filename)
        .to_str()
        .unwrap()
        .to_string()
}

pub fn test_font_dir() -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts_dir = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("fonts");
    fonts_dir.to_str().unwrap().to_string()
}

pub fn load_vl_spec_string(name: &str) -> String {
    let spec_path = vl_spec_path(name);
    fs::read_to_string(&spec_path).unwrap()
}

pub fn validate_png_header(data: &[u8]) -> bool {
    data.len() >= 8 && data[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
}

pub fn validate_jpeg_header(data: &[u8]) -> bool {
    data.len() >= 4 && data[0..4] == [0xFF, 0xD8, 0xFF, 0xE0]
}

pub fn validate_pdf_header(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == b"%PDF"
}
