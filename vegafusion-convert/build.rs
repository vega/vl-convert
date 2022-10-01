use std::path::Path;
use std::fs;
use std::process::Command;

const SKYPACK_URL: &str = "https://cdn.skypack.dev";
const VL_4_17_PATH: &str = "/pin/vega-lite@v4.17.0-ycT3UrEO81NWOPVKlbjt/mode=imports,min/optimized/vega-lite.js";
const VL_5_0_PATH: &str = "/pin/vega-lite@v5.0.0-pmBUeju4pfpuhRqteP34/mode=imports,min/optimized/vega-lite.js";
const VL_5_1_PATH: &str = "/pin/vega-lite@v5.1.1-qL3Pu0B4EEJouhGpByed/mode=imports,min/optimized/vega-lite.js";
const VL_5_2_PATH: &str = "/pin/vega-lite@v5.2.0-0lbC9JVxwLSC3btqiwR4/mode=imports,min/optimized/vega-lite.js";
const VL_5_3_PATH: &str = "/pin/vega-lite@v5.3.0-dnS8FsGfJPn0FoksPhAq/mode=imports,min/optimized/vega-lite.js";
const VL_5_4_PATH: &str = "/pin/vega-lite@v5.4.0-9xYSqs414yDb6NHwONaK/mode=imports,min/optimized/vega-lite.js";
const VL_5_5_PATH: &str = "/pin/vega-lite@v5.5.0-x3x9oTW9wvfyOekd4a63/mode=imports,min/optimized/vega-lite.js";

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

    // let vl_path = format!(
    //     "/pin/vega-lite@v{VL_VERSION}-{VL_SKYPACK_HASH}/mode=imports,min/optimized/vega-lite.js",
    //     VL_VERSION=VL_VERSION, VL_SKYPACK_HASH=VL_SKYPACK_HASH,
    // );
    //
    // let vl_url = format!(
    //     "https://cdn.skypack.dev{vl_path}",
    //     vl_path=vl_path
    // );

    fs::write(main_path, format!(
        r#"
import * as vl_4_17 from "{SKYPACK_URL}{VL_4_17_PATH}";
import * as vl_5_0 from "{SKYPACK_URL}{VL_5_0_PATH}";
import * as vl_5_1 from "{SKYPACK_URL}{VL_5_1_PATH}";
import * as vl_5_2 from "{SKYPACK_URL}{VL_5_2_PATH}";
import * as vl_5_3 from "{SKYPACK_URL}{VL_5_3_PATH}";
import * as vl_5_4 from "{SKYPACK_URL}{VL_5_4_PATH}";
import * as vl_5_5 from "{SKYPACK_URL}{VL_5_5_PATH}";
"#,
        SKYPACK_URL=SKYPACK_URL,
        VL_4_17_PATH=VL_4_17_PATH,
        VL_5_0_PATH=VL_5_0_PATH,
        VL_5_1_PATH=VL_5_1_PATH,
        VL_5_2_PATH=VL_5_2_PATH,
        VL_5_3_PATH=VL_5_3_PATH,
        VL_5_4_PATH=VL_5_4_PATH,
        VL_5_5_PATH=VL_5_5_PATH,
    )).expect("Failed to write imports.js");

    // Use deno vendor to download vega-lite and dependencies to the vendor directory
    Command::new("deno")
        .current_dir(root_path)
        .arg("vendor")
        .arg("vendor/imports.js")
        .arg("--force")
        .output()
        .expect("failed to execute deno vendor");

    //
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


    let mut content = format!(
        r#"
use std::collections::HashMap;

const SKYPACK_URL: &str = "{SKYPACK_URL}";

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum VlVersion {{
    v4_17,
    v5_0,
    v5_1,
    v5_2,
    v5_3,
    v5_4,
    v5_5,
}}

impl VlVersion {{
    pub fn to_path(self) -> String {{
        use VlVersion::*;
        let path = match self {{
            v4_17 => "{VL_4_17_PATH}",
            v5_0 => "{VL_5_0_PATH}",
            v5_1 => "{VL_5_1_PATH}",
            v5_2 => "{VL_5_2_PATH}",
            v5_3 => "{VL_5_3_PATH}",
            v5_4 => "{VL_5_4_PATH}",
            v5_5 => "{VL_5_5_PATH}",
        }};
        path.to_string()
    }}

    pub fn to_url(self) -> String {{
        format!("{{}}{{}}", SKYPACK_URL, self.to_path())
    }}
}}

pub fn build_import_map() -> HashMap<String, String> {{
    let mut m: HashMap<String, String> = HashMap::new();
"#,
        SKYPACK_URL=SKYPACK_URL,
        VL_4_17_PATH=VL_4_17_PATH,
        VL_5_0_PATH=VL_5_0_PATH,
        VL_5_1_PATH=VL_5_1_PATH,
        VL_5_2_PATH=VL_5_2_PATH,
        VL_5_3_PATH=VL_5_3_PATH,
        VL_5_4_PATH=VL_5_4_PATH,
        VL_5_5_PATH=VL_5_5_PATH,
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
    for vl_path in &[VL_4_17_PATH, VL_5_0_PATH, VL_5_1_PATH, VL_5_2_PATH, VL_5_3_PATH, VL_5_4_PATH, VL_5_5_PATH] {
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
