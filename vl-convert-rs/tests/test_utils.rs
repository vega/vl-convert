use dssim::{Dssim, DssimImage};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;
use vl_convert_rs::text::register_font_directory;

static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fonts_dir = root_path.join("tests").join("fonts");
        register_font_directory(fonts_dir.to_str().unwrap())
            .expect("Failed to register test font directory");
    });
}

pub fn load_vg_spec(name: &str) -> serde_json::Value {
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

pub fn to_dssim(img: &[u8]) -> Result<DssimImage<f32>, Box<dyn std::error::Error>> {
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    tmpfile.write_all(img).unwrap();
    dssim::load_image(&Dssim::new(), tmpfile.path())
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

pub fn make_expected_vg_png_path(name: &str) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("specs")
        .join("expected")
        .join(format!("{}.png", name))
}

pub fn load_expected_vg_png_dssim(name: &str) -> Option<DssimImage<f32>> {
    let spec_path = make_expected_vg_png_path(name);
    dssim::load_image(&Dssim::new(), spec_path).ok()
}

pub fn write_failed_vg_png(name: &str, img: &[u8]) -> PathBuf {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let failed_dir = root_path.join("tests").join("specs").join("failed");

    fs::create_dir_all(failed_dir.clone()).unwrap();

    let file_path = failed_dir.join(format!("{}.png", name));

    let mut file = fs::File::create(file_path.clone()).unwrap();
    file.write_all(img).unwrap();
    file_path
}

pub fn check_vg_png(name: &str, img: &[u8]) {
    let expected_dssim = load_expected_vg_png_dssim(name);
    if let Some(expected_dssim) = expected_dssim {
        match to_dssim(img) {
            Ok(img_dssim) => {
                let attr = Dssim::new();

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
