use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::process::Command;

const VL_PATHS: &[(&str, &str)] = &[
    (
        "4.17",
        "/pin/vega-lite@v4.17.0-ycT3UrEO81NWOPVKlbjt/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.2",
        "/pin/vega-lite@v5.2.0-0lbC9JVxwLSC3btqiwR4/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.3",
        "/pin/vega-lite@v5.3.0-dnS8FsGfJPn0FoksPhAq/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.4",
        "/pin/vega-lite@v5.4.0-9xYSqs414yDb6NHwONaK/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.5",
        "/pin/vega-lite@v5.5.0-x3x9oTW9wvfyOekd4a63/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.6",
        "/pin/vega-lite@v5.6.0-jKfgpjSz6Y52Dxyq2kpj/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.7",
        "/pin/vega-lite@v5.7.1-C1L95AD7TVhfiybpzZ1h/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.8",
        "/pin/vega-lite@v5.8.0-4snbURNltT4se5LjMOKF/mode=imports,min/optimized/vega-lite.js",
    ),
];
const SKYPACK_URL: &str = "https://cdn.skypack.dev";
const VEGA_PATH: &str = "/pin/vega@v5.25.0-r16knbfAAfBFDoUvoc7K/mode=imports,min/optimized/vega.js";
const VEGA_THEMES_PATH: &str =
    "/pin/vega-themes@v2.13.0-mG3SR6UGwwl83yUi5ncr/mode=imports,min/optimized/vega-themes.js";

// Example custom build script.
fn main() {
    // Make sure vendor directory exists
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vl_convert_rs_path = root_path.join("../").join("vl-convert-rs");
    let vendor_path = vl_convert_rs_path.join("vendor");
    if vendor_path.exists() {
        fs::remove_dir_all(&vendor_path).unwrap();
    }

    // Create main.js that includes the desired imports
    let importsjs_path = vl_convert_rs_path.join("vendor_imports.js");

    let mut imports = String::new();

    // Write Vega-Lite
    for (ver, path) in VL_PATHS {
        let ver_under = ver.replace('.', "_");
        writeln!(
            imports,
            "import * as v_{ver_under} from \"{SKYPACK_URL}{path}\";",
            ver_under = ver_under,
            SKYPACK_URL = SKYPACK_URL,
            path = path
        )
        .unwrap();
    }

    // Write Vega
    writeln!(
        imports,
        "import * as vega from \"{SKYPACK_URL}{VEGA_PATH}\";",
        SKYPACK_URL = SKYPACK_URL,
        VEGA_PATH = VEGA_PATH
    )
    .unwrap();

    // Write VegaThemes
    writeln!(
        imports,
        "import * as vegaThemes from \"{SKYPACK_URL}{VEGA_THEMES_PATH}\";",
        SKYPACK_URL = SKYPACK_URL,
        VEGA_THEMES_PATH = VEGA_THEMES_PATH
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

    // Load vendored import_map
    let import_map_path = vendor_path.join("import_map.json");
    let import_map_str =
        fs::read_to_string(import_map_path).expect("Unable to read import_map.json file");

    let import_map: serde_json::Value =
        serde_json::from_str(&import_map_str).expect("Unable to parse import_map.json file");

    let import_map = import_map.as_object().expect("Invalid JSON format");
    let scopes = import_map.get("scopes").unwrap();
    let scopes = scopes.as_object().unwrap();
    let skypack_obj = scopes.get("./cdn.skypack.dev/").unwrap();
    let skypack_obj = skypack_obj.as_object().unwrap();

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

const SKYPACK_URL: &str = "{SKYPACK_URL}";
const VEGA_PATH: &str = "{VEGA_PATH}";
const VEGA_THEMES_PATH: &str = "{VEGA_THEMES_PATH}";

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
        version_instances_csv = version_instances_csv,
        SKYPACK_URL = SKYPACK_URL,
        VEGA_PATH = VEGA_PATH,
        VEGA_THEMES_PATH = VEGA_THEMES_PATH,
        LATEST_VEGALITE = VL_PATHS[VL_PATHS.len() - 1].0
    );
    // Add packages
    for (k, v) in skypack_obj {
        // Strip trailing ? suffixes like ?from=vega
        let k = if let Some(question_inex) = k.find('?') {
            k[..question_inex].to_string()
        } else {
            k.clone()
        };

        let v = v.as_str().unwrap();
        writeln!(
            content,
            "    m.insert(\"{}\".to_string(), include_str!(\"../../vendor/{}\").to_string());",
            k, v
        )
        .unwrap();
    }

    // Add pinned packages
    // Vega-Lite
    for (_, vl_path) in VL_PATHS {
        writeln!(
            content,
            "    m.insert(\"{vl_path}\".to_string(), include_str!(\"../../vendor/cdn.skypack.dev{vl_path}\").to_string());",
            vl_path=vl_path,
        ).unwrap();
    }

    // Vega
    writeln!(
        content,
        "    m.insert(\"{VEGA_PATH}\".to_string(), include_str!(\"../../vendor/cdn.skypack.dev{VEGA_PATH}\").to_string());",
        VEGA_PATH=VEGA_PATH,
    ).unwrap();

    // Vega Themes
    writeln!(
        content,
        "    m.insert(\"{VEGA_THEMES_PATH}\".to_string(), include_str!(\"../../vendor/cdn.skypack.dev{VEGA_THEMES_PATH}\").to_string());",
        VEGA_THEMES_PATH=VEGA_THEMES_PATH,
    ).unwrap();

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
