// Allow uninlined format args in this dev utility
#![allow(clippy::uninlined_format_args)]

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
    // 5.8 is used by Altair 5.0 (keep longer)
    ("5.8", "/npm/vega-lite@5.8.0/+esm"),
    // 5.14.1 is used by Altair 5.1.0 (keep longer)
    ("5.14", "/npm/vega-lite@5.14.1/+esm"),
    // 5.15.1 is used by Altair 5.1.1 (keep longer)
    ("5.15", "/npm/vega-lite@5.15.1/+esm"),
    // 5.16.3 is used by Altair 5.2.0 (keep longer)
    ("5.16", "/npm/vega-lite@5.16.3/+esm"),
    // 5.17.0 is used by Altair 5.3.0 (keep longer)
    ("5.17", "/npm/vega-lite@5.17.0/+esm"),
    // 5.20.1 is used by Altair 5.4.0 (keep longer)
    ("5.20", "/npm/vega-lite@5.20.1/+esm"),
    ("5.21", "/npm/vega-lite@5.21.0/+esm"),
    ("6.1", "/npm/vega-lite@6.1.0/+esm"),
    ("6.4", "/npm/vega-lite@6.4.1/+esm"),
];
const JSDELIVR_URL: &str = "https://cdn.jsdelivr.net";
const VEGA_PATH: &str = "/npm/vega@6.2.0/+esm";
const VEGA_THEMES_PATH: &str = "/npm/vega-themes@3.0.0/+esm";
const VEGA_EMBED_PATH: &str = "/npm/vega-embed@7.0.2/+esm";
const DEBOUNCE_PATH: &str = "/npm/lodash.debounce@4.0.8/+esm";
const MSGPACK_PATH: &str = "/npm/@msgpack/msgpack@3.1.2/+esm";

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
        // Extract package name and version from path for esm.run URL
        let package_url = path
            .strip_prefix("/npm/")
            .unwrap()
            .strip_suffix("/+esm")
            .unwrap();
        writeln!(
            imports,
            "import * as v_{ver_under} from \"{JSDELIVR_URL}/npm/{package_url}/+esm\";",
        )
        .unwrap();
    }

    // Write Vega
    let vega_package_url = VEGA_PATH
        .strip_prefix("/npm/")
        .unwrap()
        .strip_suffix("/+esm")
        .unwrap();
    writeln!(
        imports,
        "import * as vega from \"{JSDELIVR_URL}/npm/{vega_package_url}/+esm\";",
    )
    .unwrap();

    // Write VegaThemes
    let vega_themes_package_url = VEGA_THEMES_PATH
        .strip_prefix("/npm/")
        .unwrap()
        .strip_suffix("/+esm")
        .unwrap();
    writeln!(
        imports,
        "import * as vegaThemes from \"{JSDELIVR_URL}/npm/{vega_themes_package_url}/+esm\";",
    )
    .unwrap();

    // Write Vega Embed
    let vega_embed_package_url = VEGA_EMBED_PATH
        .strip_prefix("/npm/")
        .unwrap()
        .strip_suffix("/+esm")
        .unwrap();
    writeln!(
        imports,
        "import * as vegaEmbed from \"{JSDELIVR_URL}/npm/{vega_embed_package_url}/+esm\";",
    )
    .unwrap();

    // Write debounce
    let debounce_package_url = DEBOUNCE_PATH
        .strip_prefix("/npm/")
        .unwrap()
        .strip_suffix("/+esm")
        .unwrap();
    writeln!(
        imports,
        "import lodashDebounce from \"{JSDELIVR_URL}/npm/{debounce_package_url}/+esm\";",
    )
    .unwrap();

    // Write msgpack
    let msgpack_package_url = MSGPACK_PATH
        .strip_prefix("/npm/")
        .unwrap()
        .strip_suffix("/+esm")
        .unwrap();
    writeln!(
        imports,
        "import * as msgpack from \"{JSDELIVR_URL}/npm/{msgpack_package_url}/+esm\";",
    )
    .unwrap();

    fs::write(&importsjs_path, imports).expect("Failed to write vendor_imports.js");

    // Create deno.json with vendor: true for Deno 2.x
    let deno_json_path = vl_convert_rs_path.join("deno.json");
    fs::write(&deno_json_path, r#"{ "vendor": true }"#).expect("Failed to write deno.json");

    // Use deno install to download dependencies to the vendor directory (Deno 2.x approach)
    // This replaces the deprecated `deno vendor` command
    let output = Command::new("deno")
        .current_dir(&vl_convert_rs_path)
        .arg("install")
        .arg("--entrypoint")
        .arg("vendor_imports.js")
        .arg("--reload")
        .output();

    match output {
        Ok(output) => {
            // Print stdout and stderr from deno command
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }

            // Check if command was successful
            if !output.status.success() {
                panic!(
                    "Deno install command failed with exit code: {}",
                    output.status
                );
            }
        }
        Err(err) => {
            panic!("Deno install command failed: {}", err);
        }
    }

    // Clean up temporary deno.json and deno.lock
    fs::remove_file(&deno_json_path).ok();
    fs::remove_file(vl_convert_rs_path.join("deno.lock")).ok();

    // Build mapping from actual file paths to canonical import paths
    // Deno 2.x uses hashed filenames like "#+esm_3b53f.js" and truncated directory names
    // We need to map these back to canonical paths like "/npm/vega@6.2.0/+esm"
    let path_mapping = build_path_mapping(&vendor_path);

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

    // Collect info on transitive dependency packages
    // We use this to detect and remove duplicate versions of transitive dependencies
    let mut packages_info: HashMap<String, Vec<(Version, String, String)>> = HashMap::new();

    // Use the path mapping to extract package info (handles Deno 2's hashed names)
    for (actual_path, canonical_path) in &path_mapping {
        // canonical_path is like "/npm/vega@6.2.0/+esm"
        if let Some(pkg_part) = canonical_path.strip_prefix("/npm/") {
            if let Some(pkg_name_ver) = pkg_part.strip_suffix("/+esm") {
                if let Some((name, ver_str)) = pkg_name_ver.rsplit_once('@') {
                    if let Ok(version) = Version::parse(ver_str) {
                        // actual_path is like "cdn.jsdelivr.net/npm/vega@6.2.0/#+esm_3b53f.js"
                        // Extract the directory name from actual_path
                        let parts: Vec<&str> = actual_path.split('/').collect();
                        if parts.len() >= 3 {
                            let dir_name = parts[2].to_string(); // e.g., "vega@6.2.0" or "#fast-json-stable-str_a4987"
                            packages_info.entry(name.to_string()).or_default().push((
                                version,
                                dir_name,
                                pkg_name_ver.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    let mut replacements: HashMap<String, String> = HashMap::new();
    let mut final_package_versions: HashMap<String, String> = HashMap::new();

    for (name, v) in packages_info.iter_mut() {
        // Sort packages in descending order by version
        v.sort_by(|a, b| b.0.cmp(&a.0));

        // Store the final version that will be kept (canonical name like "vega@6.2.0")
        if !v.is_empty() {
            final_package_versions.insert(name.clone(), v[0].2.clone());
        }

        // For packages other than vega-lite, if there are multiple versions of the same package
        // delete the older ones and store the import string replacement to apply to other files
        if name != "vega-lite" && v.len() > 1 {
            for i in 1..v.len() {
                // v[i].2 is canonical name like "vega@5.25.0", v[0].2 is "vega@6.2.0"
                replacements.insert(v[i].2.clone(), v[0].2.clone());
                // v[i].1 is actual directory name (may be hashed like "#fast-json-stable-str_a4987")
                let file_path = format!("{vendor_path_str}/cdn.jsdelivr.net/npm/{}", v[i].1);
                fs::remove_dir_all(file_path).unwrap_or(());
            }
        }
    }

    // Update version constants based on actual available packages
    let actual_vega_version = final_package_versions
        .get("vega")
        .map(|v| format!("/npm/{}/+esm", v))
        .unwrap_or_else(|| VEGA_PATH.to_string());
    let actual_vega_themes_version = final_package_versions
        .get("vega-themes")
        .map(|v| format!("/npm/{}/+esm", v))
        .unwrap_or_else(|| VEGA_THEMES_PATH.to_string());
    let actual_vega_embed_version = final_package_versions
        .get("vega-embed")
        .map(|v| format!("/npm/{}/+esm", v))
        .unwrap_or_else(|| VEGA_EMBED_PATH.to_string());
    let actual_debounce_version = final_package_versions
        .get("lodash.debounce")
        .map(|v| format!("/npm/{}/+esm", v))
        .unwrap_or_else(|| DEBOUNCE_PATH.to_string());
    let actual_msgpack_version = final_package_versions
        .get("@msgpack/msgpack")
        .map(|v| format!("/npm/{}/+esm", v))
        .unwrap_or_else(|| MSGPACK_PATH.to_string());

    let mut content = format!(
        r#"
// *************************************************************************
// * This file is generated by vl-convert-vendor/src/main.rs. Do not edit! *
// *************************************************************************
use std::collections::HashMap;
use std::str::FromStr;
use deno_runtime::deno_core::anyhow::bail;
use deno_runtime::deno_core::error::AnyError;

pub const JSDELIVR_URL: &str = "{JSDELIVR_URL}";
pub const VEGA_PATH: &str = "{VEGA_PATH}";
pub const VEGA_THEMES_PATH: &str = "{VEGA_THEMES_PATH}";
pub const VEGA_EMBED_PATH: &str = "{VEGA_EMBED_PATH}";
pub const DEBOUNCE_PATH: &str = "{DEBOUNCE_PATH}";
pub const MSGPACK_PATH: &str = "{MSGPACK_PATH}";

pub const VEGA_VERSION: &str = "{VEGA_VERSION}";
pub const VEGA_THEMES_VERSION: &str = "{VEGA_THEMES_VERSION}";
pub const VEGA_EMBED_VERSION: &str = "{VEGA_EMBED_VERSION}";

pub fn url_for_path(path: &str) -> String {{
    format!("{{}}{{}}", JSDELIVR_URL, path)
}}

pub fn vega_url() -> String {{
    url_for_path(VEGA_PATH)
}}

pub fn vega_themes_url() -> String {{
    url_for_path(VEGA_THEMES_PATH)
}}

pub fn msgpack_url() -> String {{
    url_for_path(MSGPACK_PATH)
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
        format!("{{}}{{}}", JSDELIVR_URL, self.to_path())
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
        JSDELIVR_URL = JSDELIVR_URL,
        VEGA_PATH = actual_vega_version,
        VEGA_VERSION = actual_vega_version
            .split("@")
            .nth(1)
            .unwrap()
            .split("/")
            .next()
            .unwrap(),
        VEGA_THEMES_VERSION = actual_vega_themes_version
            .split("@")
            .nth(1)
            .unwrap()
            .split("/")
            .next()
            .unwrap(),
        VEGA_EMBED_VERSION = actual_vega_embed_version
            .split("@")
            .nth(1)
            .unwrap()
            .split("/")
            .next()
            .unwrap(),
        VEGA_THEMES_PATH = actual_vega_themes_version,
        VEGA_EMBED_PATH = actual_vega_embed_version,
        DEBOUNCE_PATH = actual_debounce_version,
        MSGPACK_PATH = actual_msgpack_version,
        LATEST_VEGALITE = VL_PATHS[VL_PATHS.len() - 1].0
    );

    // Perform import replacements in remaining files
    visit_dirs(&vendor_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        replace_in_file(&p, &replacements).unwrap();
    })
    .unwrap();

    // Rebuild path mapping after deletions
    let path_mapping = build_path_mapping(&vendor_path);

    // Write include_str! statements to inline source code in our executable
    // Use canonical paths as keys but actual file paths for include_str!()
    let mut sorted_mappings: Vec<_> = path_mapping.iter().collect();
    sorted_mappings.sort_by_key(|(actual, _)| *actual);

    for (actual_path, canonical_path) in sorted_mappings {
        // actual_path is like "cdn.jsdelivr.net/npm/vega@6.2.0/#+esm_3b53f.js"
        // canonical_path is like "/npm/vega@6.2.0/+esm"
        writeln!(
            content,
            "    m.insert(\"{canonical_path}\".to_string(), include_str!(\"../../vendor/{actual_path}\").to_string());",
        )
        .unwrap();
    }

    content.push_str("    m\n}\n");

    // Overwrite with patched files
    // Patched files use canonical paths, so we need to map them to actual paths
    let patched_dir = root_path.join("patched");
    if patched_dir.exists() {
        // Build reverse mapping: canonical path -> actual path
        let reverse_mapping: HashMap<String, String> = path_mapping
            .iter()
            .map(|(actual, canonical)| (canonical.clone(), actual.clone()))
            .collect();

        visit_dirs(&patched_dir, &mut |f| {
            let p = f.path().canonicalize().unwrap();
            let patched_path_str = patched_dir.to_str().unwrap();
            let relative = &p.to_str().unwrap()[(patched_path_str.len())..];
            // relative is like "/cdn.jsdelivr.net/npm/vega@6.2.0/+esm.js"
            // Convert to canonical path format: "/npm/vega@6.2.0/+esm"
            if let Some(canonical) = relative
                .strip_prefix("/cdn.jsdelivr.net")
                .and_then(|s| s.strip_suffix(".js"))
            {
                if let Some(actual_path) = reverse_mapping.get(canonical) {
                    let vendored_file_path = vendor_path.join(actual_path);
                    fs::copy(&p, vendored_file_path).unwrap();
                }
            }
        })
        .unwrap();
    }

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

#[allow(deprecated)] // zip_extract::extract is deprecated but still works
fn download_locales(url: &str, output_dir: &PathBuf) -> Result<(), AnyError> {
    let response = reqwest::blocking::get(url)?;
    let archive_bytes = response.bytes().unwrap();

    let temp_dir = TempDir::new()?;
    zip_extract::extract(Cursor::new(archive_bytes), temp_dir.path(), true)?;

    let temp_path_locale = temp_dir.path().join("locale");
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

/// Build a mapping from actual file paths (with Deno 2's hashed names) to canonical import paths.
/// Returns HashMap where:
/// - Key: actual relative path like "cdn.jsdelivr.net/npm/vega@6.2.0/#+esm_3b53f.js"
/// - Value: canonical path like "/npm/vega@6.2.0/+esm"
fn build_path_mapping(vendor_path: &Path) -> HashMap<String, String> {
    let mut mapping = HashMap::new();
    let vendor_path_str = vendor_path.to_str().unwrap();

    visit_dirs(vendor_path, &mut |f| {
        let p = f.path();
        if p.extension().map(|e| e == "js").unwrap_or(false) {
            let p = p.canonicalize().unwrap();
            let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];

            // Only process files under cdn.jsdelivr.net
            if !relative.starts_with("cdn.jsdelivr.net") {
                return;
            }

            // Try to extract canonical path from "Original file" comment
            if let Some(canonical) = extract_canonical_path(&p) {
                mapping.insert(relative.to_string(), canonical);
            }
        }
    })
    .unwrap();

    mapping
}

/// Extract the canonical import path from a JS file's "Original file" comment.
/// Returns path like "/npm/vega@6.2.0/+esm" from comment "Original file: /npm/vega@6.2.0/build/vega.module.js"
fn extract_canonical_path(file_path: &Path) -> Option<String> {
    let content = fs::read_to_string(file_path).ok()?;

    // Look for: " * Original file: /npm/package-name@version/..."
    for line in content.lines().take(10) {
        if let Some(rest) = line.strip_prefix(" * Original file: ") {
            // rest is like "/npm/vega@6.2.0/build/vega.module.js"
            // We want to extract "/npm/vega@6.2.0/+esm"
            if let Some(npm_path) = rest.strip_prefix("/npm/") {
                let pkg_name_ver = if npm_path.starts_with('@') {
                    let mut parts = npm_path.splitn(3, '/');
                    let scope = parts.next()?;
                    let package = parts.next()?;
                    format!("{scope}/{package}")
                } else {
                    npm_path.split('/').next()?.to_string()
                };
                return Some(format!("/npm/{}/+esm", pkg_name_ver));
            }
        }
    }
    None
}
