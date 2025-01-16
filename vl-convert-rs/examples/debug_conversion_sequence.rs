use vl_convert_rs::{VlConverter};

#[tokio::main]
async fn main() {
    convert().await;
    convert().await;
}

async fn convert() {
    // println!("CARGO_MANIFEST_DIR: {:?}", env!("CARGO_MANIFEST_DIR"));
    // let main_module =
    //     deno_core::resolve_path("vendor_imports.js", Path::new(env!("CARGO_MANIFEST_DIR")))
    //         .unwrap();

    // println!("main_module: {:?}", main_module);

    println!("VlConverter::new()");
    let mut converter = VlConverter::new();
    println!("converter.does_it_crash()");
    converter.does_it_crash().await.unwrap();

    // converter
    //     .vegalite_to_svg(
    //         vl_spec,
    //         VlOpts {
    //             vl_version: VlVersion::v5_8,
    //             ..Default::default()
    //         },
    //     )
    //     .await
    //     .expect("Failed to perform Vega-Lite to Vega conversion")
}
