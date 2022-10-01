use std::path::Path;
use std::fs;
use std::process::Command;

const VL_PATHS: &[(&str, &str)] = &[
    ("4.17", "/pin/vega-lite@v4.17.0-ycT3UrEO81NWOPVKlbjt/mode=imports,min/optimized/vega-lite.js"),
    ("5.0", "/pin/vega-lite@v5.0.0-pmBUeju4pfpuhRqteP34/mode=imports,min/optimized/vega-lite.js"),
    ("5.1", "/pin/vega-lite@v5.1.1-qL3Pu0B4EEJouhGpByed/mode=imports,min/optimized/vega-lite.js"),
    ("5.2", "/pin/vega-lite@v5.2.0-0lbC9JVxwLSC3btqiwR4/mode=imports,min/optimized/vega-lite.js"),
    ("5.3", "/pin/vega-lite@v5.3.0-dnS8FsGfJPn0FoksPhAq/mode=imports,min/optimized/vega-lite.js"),
    ("5.4", "/pin/vega-lite@v5.4.0-9xYSqs414yDb6NHwONaK/mode=imports,min/optimized/vega-lite.js"),
    ("5.5", "/pin/vega-lite@v5.5.0-x3x9oTW9wvfyOekd4a63/mode=imports,min/optimized/vega-lite.js"),
];
const SKYPACK_URL: &str = "https://cdn.skypack.dev";

// Example custom build script.
fn main() {
    // Make sure vendor directory exists
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vendor_path = root_path.join("vendor");

    if vendor_path.exists() {
       fs::remove_dir_all(&vendor_path).unwrap();
    }
    fs::create_dir(&vendor_path).unwrap();

    // Create main.js that includes the desired imports
    let main_path = vendor_path.join("imports.js");

    let mut imports = String::new();
    for (ver, path) in VL_PATHS {
        let ver_under = ver.replace(".", "_");
        imports.push_str(&format!(
            "import * as v_{ver_under} from \"{SKYPACK_URL}{path}\";\n",
            ver_under=ver_under,
            SKYPACK_URL=SKYPACK_URL,
            path=path,
        ))
    }
    fs::write(main_path, imports).expect("Failed to write imports.js");

    // Use deno vendor to download vega-lite and dependencies to the vendor directory
    Command::new("deno")
        .current_dir(root_path)
        .arg("vendor")
        .arg("vendor/imports.js")
        .arg("--force")
        .output()
        .expect("failed to execute deno vendor");

    // Load vendored import_map
    let import_map_path = vendor_path.join("import_map.json");
    let import_map_str = fs::read_to_string(&import_map_path)
        .expect("Unable to read import_map.json file");

    let import_map: serde_json::Value = serde_json::from_str(&import_map_str)
        .expect("Unable to parse import_map.json file");

    let import_map = import_map.as_object().expect("Invalid JSON format");
    let scopes = import_map.get("scopes").unwrap();
    let scopes = scopes.as_object().unwrap();
    let skypack_obj = scopes.get("./cdn.skypack.dev/").unwrap();
    let skypack_obj = skypack_obj.as_object().unwrap();

    // Write import_map.rs file
    // Build versions csv
    let ver_unders: Vec<_> = VL_PATHS.iter().map(|(ver, _)| {
        format!("v{}", ver.replace(".", "_"))
    }).collect();
    let vl_versions_csv = ver_unders.join(",\n    ");

    // Path match csv
    let ver_path_matches: Vec<_> = VL_PATHS.iter().map(|(ver, path)| {
        format!(
            "v{} => \"{}\"",
            ver.replace(".", "_"),
            path
        )
    }).collect();
    let path_match_csv = ver_path_matches.join(",\n            ");

    // FromStr match csv
    let from_str_matches: Vec<_> = VL_PATHS.iter().map(|(ver, _)| {
        let ver_under = ver.replace(".", "_");
        format!(
            "\"{ver}\" | \"v{ver}\" | \"{ver_under}\" | \"v{ver_under}\" => Self::v{ver_under}",
            ver=ver, ver_under=ver_under
        )
    }).collect();
    let from_str_matches_csv = from_str_matches.join(",\n            ");

    let mut content = format!(
        r#"
use std::collections::HashMap;
use std::str::FromStr;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

const SKYPACK_URL: &str = "{SKYPACK_URL}";

#[derive(Debug, Copy, Clone)]
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


impl FromStr for VlVersion {{
    type Err = AnyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {{
        Ok(match s {{
            {from_str_matches_csv},
            _ => bail!("Unsupport Vega-Lite version string {{}}", s)
        }})
    }}
}}

pub fn build_import_map() -> HashMap<String, String> {{
    let mut m: HashMap<String, String> = HashMap::new();
"#,
        vl_versions_csv=vl_versions_csv,
        path_match_csv=path_match_csv,
        from_str_matches_csv=from_str_matches_csv,
        SKYPACK_URL=SKYPACK_URL,
    );
    // Add packages
    for (k, v) in skypack_obj {
        let v = v.as_str().unwrap();
        content.push_str(&format!(
            "    m.insert(\"{}\".to_string(), include_str!(\"../../vendor/{}\").to_string());\n",
            k,
            v
        ))
    }

    // Add pinned packages
    // Vega-Lite
    for (_, vl_path) in VL_PATHS {
        content.push_str(&format!(
            "    m.insert(\"{vl_path}\".to_string(), include_str!(\"../../vendor/cdn.skypack.dev{vl_path}\").to_string());\n",
            vl_path=vl_path,
        ));
    }

    content.push_str("    m\n}\n");
    let deno_deps_path = root_path.join("src").join("module_loader");

    fs::write(deno_deps_path.join("import_map.rs"), content).unwrap();

//     // // Tell Cargo that if the given file changes, to rerun this build script.
//     // println!("cargo:rerun-if-changed=src/hello.c");
//     // // Use the `cc` crate to build a C file and statically link it.
//     // cc::Build::new()
//     //     .file("src/hello.c")
//     //     .compile("hello");
}
