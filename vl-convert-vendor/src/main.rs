use anyhow::Error as AnyError;
use dircpy::copy_dir;
use semver::Version;
use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::fs::DirEntry;
use std::io;
use std::io::{Cursor, Read, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const VL_PATHS: &[(&str, &str)] = &[
    // 4.17 is used by Altair 4.2 (keep forever)
    (
        "4.17",
        "/pin/vega-lite@v4.17.0-ycT3UrEO81NWOPVKlbjt/mode=imports,min/optimized/vega-lite.js",
    ),
    // 5.8 is used by Altair 5.0 (keep forever)
    (
        "5.8",
        "/pin/vega-lite@v5.8.0-4snbURNltT4se5LjMOKF/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.9",
        "/pin/vega-lite@v5.9.3-QyXScylQe0TTmb9DRCES/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.10",
        "/pin/vega-lite@v5.10.0-Vm0dgr6cpOyUiTjlPzt9/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.11",
        "/pin/vega-lite@v5.11.1-Q5Jhmb2acmWm03IObXvn/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.12",
        "/pin/vega-lite@v5.12.0-ujK64YZaLHcwzRN5lx1E/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.13",
        "/pin/vega-lite@v5.13.0-GkFo6HVxfKtvVL5RV8aE/mode=imports,min/optimized/vega-lite.js",
    ),
    // 5.14.1 is used by Altair 5.1.0 (keep forever)
    (
        "5.14",
        "/pin/vega-lite@v5.14.1-0IRM1VigcIVzRzBRoLFR/mode=imports,min/optimized/vega-lite.js",
    ),
    // 5.15.1 is used by Altair 5.1.1 (keep forever)
    (
        "5.15",
        "/pin/vega-lite@v5.15.1-lQeQs8sDPgFa9d7Jm3sd/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.16",
        "/pin/vega-lite@v5.16.3-Hw7pZxUuaiVgThsNMjY9/mode=imports,min/optimized/vega-lite.js",
    ),
];
const SKYPACK_URL: &str = "https://cdn.skypack.dev";
const VEGA_PATH: &str = "/pin/vega@v5.26.1-qzT1gQErRVzfnh254DSg/mode=imports,min/optimized/vega.js";
const VEGA_THEMES_PATH: &str =
    "/pin/vega-themes@v2.14.0-RvUmNETlVH2y3yQM1y36/mode=imports,min/optimized/vega-themes.js";
const VEGA_EMBED_PATH: &str =
    "/pin/vega-embed@v6.23.0-Fpmq39rehEH8HWtd6nzv/mode=imports,min/optimized/vega-embed.js";
const DEBOUNCE_PATH: &str =
    "/pin/lodash.debounce@v4.0.8-aOLIwnE2RethWPrEzTeR/mode=imports,min/optimized/lodash.debounce.js";

// Example custom build script.
fn main() {
    // Make sure vendor directory exists
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vl_convert_rs_path = root_path.join("../").join("vl-convert-rs");
    let vendor_path = vl_convert_rs_path.canonicalize().unwrap().join("vendor");
    let format_locales_path = vl_convert_rs_path
        .join("locales")
        .join("format")
        .canonicalize()
        .unwrap();
    let time_format_locales_path = vl_convert_rs_path
        .join("locales")
        .join("time-format")
        .canonicalize()
        .unwrap();
    let vendor_path_str = vendor_path.to_str().unwrap();
    if vendor_path.exists() {
        fs::remove_dir_all(&vendor_path).unwrap();
    }

    // Download locales
    download_locales(
        "https://github.com/d3/d3-format/archive/refs/heads/main.zip",
        &vl_convert_rs_path.join("locales").join("format"),
    )
    .unwrap();
    download_locales(
        "https://github.com/d3/d3-time-format/archive/refs/heads/main.zip",
        &vl_convert_rs_path.join("locales").join("time-format"),
    )
    .unwrap();

    // Create main.js that includes the desired imports
    let importsjs_path = vl_convert_rs_path.join("vendor_imports.js");

    let mut imports = String::new();

    // Write Vega-Lite
    for (ver, path) in VL_PATHS {
        let ver_under = ver.replace('.', "_");
        writeln!(
            imports,
            "import * as v_{ver_under} from \"{SKYPACK_URL}{path}\";",
        )
        .unwrap();
    }

    // Write Vega
    writeln!(
        imports,
        "import * as vega from \"{SKYPACK_URL}{VEGA_PATH}\";",
    )
    .unwrap();

    // Write VegaThemes
    writeln!(
        imports,
        "import * as vegaThemes from \"{SKYPACK_URL}{VEGA_THEMES_PATH}\";",
    )
    .unwrap();

    // Write Vega Embed
    writeln!(
        imports,
        "import * as vegaEmbed from \"{SKYPACK_URL}{VEGA_EMBED_PATH}\";",
    )
    .unwrap();

    // Write debounce
    writeln!(
        imports,
        "import lodashDebounce from \"{SKYPACK_URL}{DEBOUNCE_PATH}\";",
    )
    .unwrap();

    fs::write(importsjs_path, imports).expect("Failed to write vendor_imports.js");

    // Use deno vendor to download vega-lite and dependencies to the vendor directory
    if let Err(err) = Command::new("deno")
        .current_dir(&vl_convert_rs_path)
        .arg("vendor")
        .arg("vendor_imports.js")
        .arg("--reload")
        .output()
    {
        panic!("Deno vendor command failed: {}", err);
    }

    // Write import_map.rs file
    // Build versions csv
    let ver_unders: Vec<_> = VL_PATHS
        .iter()
        .map(|(ver, _)| format!("v{}", ver.replace('.', "_")))
        .collect();
    let vl_versions_csv = ver_unders.join(",\n    ");

    // Path match csv
    let ver_path_matches: Vec<_> = VL_PATHS
        .iter()
        .map(|(ver, path)| format!("v{} => \"{}\"", ver.replace('.', "_"), path))
        .collect();
    let path_match_csv = ver_path_matches.join(",\n            ");

    // FromStr match csv
    let from_str_matches: Vec<_> = VL_PATHS
        .iter()
        .map(|(ver, _)| {
            let ver_under = ver.replace('.', "_");
            format!(
                "\"{ver}\" | \"v{ver}\" | \"{ver_under}\" | \"v{ver_under}\" => Self::v{ver_under}",
                ver = ver,
                ver_under = ver_under
            )
        })
        .collect();
    let from_str_matches_csv = from_str_matches.join(",\n            ");

    // To semver match csv
    let to_semver_matches: Vec<_> = VL_PATHS
        .iter()
        .map(|(ver, _)| format!("v{} => \"{}\"", ver.replace('.', "_"), ver))
        .collect();
    let to_semver_match_csv = to_semver_matches.join(",\n            ");

    // Variants csv
    let version_instances: Vec<_> = VL_PATHS
        .iter()
        .map(|(ver, _)| {
            let ver_under = ver.replace('.', "_");
            format!("VlVersion::v{ver_under}", ver_under = ver_under)
        })
        .collect();
    let version_instances_csv = version_instances.join(",\n    ");

    let mut content = format!(
        r#"
// *************************************************************************
// * This file is generated by vl-convert-vendor/src/main.rs. Do not edit! *
// *************************************************************************
use std::collections::HashMap;
use std::str::FromStr;
use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::error::AnyError;

pub const SKYPACK_URL: &str = "{SKYPACK_URL}";
pub const VEGA_PATH: &str = "{VEGA_PATH}";
pub const VEGA_THEMES_PATH: &str = "{VEGA_THEMES_PATH}";
pub const VEGA_EMBED_PATH: &str = "{VEGA_EMBED_PATH}";
pub const DEBOUNCE_PATH: &str = "{DEBOUNCE_PATH}";

pub fn url_for_path(path: &str) -> String {{
    format!("{{}}{{}}", SKYPACK_URL, path)
}}

pub fn vega_url() -> String {{
    url_for_path(VEGA_PATH)
}}

pub fn vega_themes_url() -> String {{
    url_for_path(VEGA_THEMES_PATH)
}}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[allow(non_camel_case_types)]
pub enum VlVersion {{
    {vl_versions_csv}
}}

impl VlVersion {{
    pub fn to_path(self) -> String {{
        use VlVersion::*;
        let path = match self {{
            {path_match_csv}
        }};
        path.to_string()
    }}

    pub fn to_url(self) -> String {{
        format!("{{}}{{}}", SKYPACK_URL, self.to_path())
    }}

    pub fn to_semver(self) -> &'static str {{
        use VlVersion::*;
        match self {{
            {to_semver_match_csv}
        }}
    }}
}}

impl Default for VlVersion {{
    fn default() -> Self {{
        VlVersion::from_str("{LATEST_VEGALITE}").unwrap()
    }}
}}

impl FromStr for VlVersion {{
    type Err = AnyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {{
        Ok(match s {{
            {from_str_matches_csv},
            _ => bail!("Unsupported Vega-Lite version string {{}}", s)
        }})
    }}
}}

pub const VL_VERSIONS: &[VlVersion] = &[
    {version_instances_csv},
];

pub fn build_import_map() -> HashMap<String, String> {{
    let mut m: HashMap<String, String> = HashMap::new();
"#,
        vl_versions_csv = vl_versions_csv,
        path_match_csv = path_match_csv,
        from_str_matches_csv = from_str_matches_csv,
        to_semver_match_csv = to_semver_match_csv,
        version_instances_csv = version_instances_csv,
        SKYPACK_URL = SKYPACK_URL,
        VEGA_PATH = VEGA_PATH,
        VEGA_THEMES_PATH = VEGA_THEMES_PATH,
        VEGA_EMBED_PATH = VEGA_EMBED_PATH,
        LATEST_VEGALITE = VL_PATHS[VL_PATHS.len() - 1].0
    );

    // Collect info on transitive dependency packages
    // We use this to detect and remove duplicate versions of transitive dependencies
    let mut packages_info: HashMap<String, Vec<(Version, String)>> = HashMap::new();
    visit_dirs(&vendor_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];
        if let Some(relative_sub) = relative.strip_prefix("cdn.skypack.dev/-/") {
            if let Some((name, rest)) = relative_sub.split_once('@') {
                if let Some((version_str, _)) = rest.strip_prefix('v').unwrap().split_once('-') {
                    let version = Version::parse(version_str).unwrap();
                    packages_info
                        .entry(name.to_string())
                        .or_default()
                        .push((version, relative_sub.to_string()));
                }
            }
        }
    })
    .unwrap();

    let mut replacements: HashMap<String, String> = HashMap::new();
    for (name, v) in packages_info.iter_mut() {
        // Sort packages in descending order by version
        v.sort_by(|a, b| b.0.cmp(&a.0));

        // For packages other than vega-lite, if there are multiple versions of the same package
        // delete the older ones and store the import string replacement to apply to other files
        if name != "vega-lite" && v.len() > 1 {
            for i in 1..v.len() {
                replacements.insert(v[i].1.clone(), v[0].1.clone());
                let file_path = format!("{vendor_path_str}/cdn.skypack.dev/-/{}", v[i].1);
                fs::remove_file(file_path).unwrap();
            }
        }
    }

    // Perform import replacements in remaining files
    visit_dirs(&vendor_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        replace_in_file(&p, &replacements).unwrap();
    })
    .unwrap();

    // Write include_str! statements to inline source code in our executable
    let skypack_domain = "cdn.skypack.dev";
    visit_dirs(&vendor_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];
        if let Some(relative_sub) = relative.strip_prefix(skypack_domain) {
            writeln!(
                content,
                "    m.insert(\"{relative_sub}\".to_string(), include_str!(\"../../vendor/{skypack_domain}{relative_sub}\").to_string());",
            )
                .unwrap();
        }
    }).unwrap();

    content.push_str("    m\n}\n");

    // Overwrite with patched files
    visit_dirs(&vendor_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];
        let patched_file_path = root_path.join("patched").join(relative);
        if patched_file_path.exists() {
            let vendored_file_path = vendor_path.join(relative);
            fs::copy(patched_file_path, vendored_file_path).unwrap();
        }
    })
    .unwrap();

    // Write locale maps
    writeln!(
        content,
        "
pub fn build_format_locale_map() -> HashMap<String, String> {{
    let mut m: HashMap<String, String> = HashMap::new();"
    )
    .unwrap();

    visit_dirs(&format_locales_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];
        if let Some(relative_sub) = relative.strip_prefix("/format/").and_then(|f| f.strip_suffix(".json")) {
            writeln!(
                content,
                "    m.insert(\"{relative_sub}\".to_string(), include_str!(\"../../locales/format/{relative_sub}.json\").to_string());",
            )
                .unwrap();
        }
    }).unwrap();

    content.push_str("    m\n}\n");

    writeln!(
        content,
        "
pub fn build_time_format_locale_map() -> HashMap<String, String> {{
    let mut m: HashMap<String, String> = HashMap::new();"
    )
    .unwrap();

    visit_dirs(&time_format_locales_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];
        if let Some(relative_sub) = relative.strip_prefix("/time-format/").and_then(|f| f.strip_suffix(".json")) {
            writeln!(
                content,
                "    m.insert(\"{relative_sub}\".to_string(), include_str!(\"../../locales/time-format/{relative_sub}.json\").to_string());",
            )
                .unwrap();
        }
    }).unwrap();

    content.push_str("    m\n}\n");

    // Write to import_map.rs in vl-convert-rs crate
    let deno_deps_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("src")
        .join("module_loader");

    let import_map_path = deno_deps_path.join("import_map.rs");
    fs::write(&import_map_path, content).unwrap();

    // Run rustfmt on import_map.rs
    if let Err(err) = Command::new("rustfmt")
        .arg(import_map_path.to_str().unwrap())
        .output()
    {
        panic!("rustfmt command failed: {}", err);
    }
}

fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&DirEntry)) -> io::Result<()> {
    if dir.is_dir() {
        let mut entries = fs::read_dir(dir)?
            // .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()?;

        entries.sort_by_key(|d| d.path());

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}

fn download_locales(url: &str, output_dir: &PathBuf) -> Result<(), AnyError> {
    let response = reqwest::blocking::get(url)?;
    let archive_bytes = response.bytes().unwrap();

    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.into_path();
    zip_extract::extract(Cursor::new(archive_bytes), &temp_path, true)?;

    let temp_path_locale = temp_path.join("locale");
    copy_dir(temp_path_locale, output_dir)?;

    Ok(())
}

fn replace_in_file(file_path: &PathBuf, replacements: &HashMap<String, String>) -> io::Result<()> {
    // Read the file content
    let mut content = String::new();
    fs::File::open(file_path)?.read_to_string(&mut content)?;

    // Apply replacements
    for (target, replacement) in replacements {
        content = content.replace(target, replacement);
    }

    // Write the modified content back to the file
    let mut file = fs::File::create(file_path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}
