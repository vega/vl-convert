use std::path::Path;
use std::fs;
use std::process::Command;

const VL_VERSION: &str = "5.2.0";
const VL_SKYPACK_HASH: &str = "0lbC9JVxwLSC3btqiwR4";

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
    fs::write(main_path, format!(r#"
import * as vl from "https://cdn.skypack.dev/pin/vega-lite@v{VL_VERSION}-{VL_SKYPACK_HASH}/mode=imports,min/optimized/vega-lite.js";
    "#, VL_VERSION=VL_VERSION, VL_SKYPACK_HASH=VL_SKYPACK_HASH,
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

    let mut content = r#"
use std::collections::HashMap;

pub fn build_import_map() -> HashMap<String, String> {
    let mut m: HashMap<String, String> = HashMap::new();
"#.to_string();

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
    let vl_path = format!(
        "/pin/vega-lite@v{VL_VERSION}-{VL_SKYPACK_HASH}/mode=imports,min/optimized/vega-lite.js",
        VL_VERSION=VL_VERSION, VL_SKYPACK_HASH=VL_SKYPACK_HASH,
    );
    content.push_str(&format!(
        "    m.insert(\"{vl_path}\".to_string(), include_str!(\"../../vendor/cdn.skypack.dev{vl_path}\").to_string());\n",
        vl_path=vl_path,
    ));

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
