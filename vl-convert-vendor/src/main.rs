use std::fmt::Write;
use std::fs;
use std::fs::DirEntry;
use std::io;
use std::path::Path;
use std::process::Command;

const VL_PATHS: &[(&str, &str)] = &[
    // 4.17 is used by Altair 4.2 (keep forever)
    (
        "4.17",
        "/pin/vega-lite@v4.17.0-ycT3UrEO81NWOPVKlbjt/mode=imports,min/optimized/vega-lite.js",
    ),
    (
        "5.7",
        "/pin/vega-lite@v5.7.1-C1L95AD7TVhfiybpzZ1h/mode=imports,min/optimized/vega-lite.js",
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
];
const SKYPACK_URL: &str = "https://cdn.skypack.dev";
const VEGA_PATH: &str = "/pin/vega@v5.25.0-r16knbfAAfBFDoUvoc7K/mode=imports,min/optimized/vega.js";
const VEGA_THEMES_PATH: &str =
    "/pin/vega-themes@v2.14.0-RvUmNETlVH2y3yQM1y36/mode=imports,min/optimized/vega-themes.js";
const VEGA_EMBED_PATH: &str =
    "/pin/vega-embed@v6.23.0-Fpmq39rehEH8HWtd6nzv/mode=imports,min/optimized/vega-embed.js";
const DEBOUNCE_PATH: &str =
    "/pin/lodash.debounce@v4.0.8-aOLIwnE2RethWPrEzTeR/mode=imports,min/optimized/lodash.debounce.js";
const VEGA_SCHEMA_PATH: &str =
    "/pin/vega-schema-url-parser@v2.2.0-YmXJGRcKOXOac3VG4xfw/mode=imports,min/optimized/vega-schema-url-parser.js";
const ARROW_PATH: &str =
    "/pin/apache-arrow@v13.0.0-MxNe6rzoVb4I5ULOjvod/mode=imports,min/optimized/apache-arrow.js";

// Example custom build script.
fn main() {
    // Make sure vendor directory exists
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vl_convert_rs_path = root_path.join("../").join("vl-convert-rs");
    let vendor_path = vl_convert_rs_path.join("vendor").canonicalize().unwrap();
    let vendor_path_str = vendor_path.to_str().unwrap();
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

    // Write vega-schema
    writeln!(
        imports,
        "import vegaSchemaUrlParser from \"{SKYPACK_URL}{VEGA_SCHEMA_PATH}\";",
    )
    .unwrap();

    // Write apache-arrow
    writeln!(
        imports,
        "import * as arrow from \"{SKYPACK_URL}{ARROW_PATH}\";",
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
pub const VEGA_SCHEMA_PATH: &str = "{VEGA_SCHEMA_PATH}";
pub const ARROW_PATH: &str = "{ARROW_PATH}";

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

    // Write include_str! statements to inline source code in our executable
    let skypack_domain = "cdn.skypack.dev";
    visit_dirs(&vendor_path, &mut |f| {
        let p = f.path().canonicalize().unwrap();
        let relative = &p.to_str().unwrap()[(vendor_path_str.len() + 1)..];
        if relative.starts_with(skypack_domain) {
            let relative_sub = &relative[skypack_domain.len()..];
            writeln!(
                content,
                "    m.insert(\"{relative_sub}\".to_string(), include_str!(\"../../vendor/{skypack_domain}/{relative_sub}\").to_string());",
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

// one possible implementation of walking a directory only visiting files
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
