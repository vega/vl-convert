pub mod converter;
pub mod module_loader;
pub use converter::VlConverter;
pub use deno_core::anyhow;
pub use serde_json;

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//         let m = module_loader::import_map::build_import_map();
//         println!("{:#?}", m.keys());
//
//         let source = m.get("/-/vega-force@v4.0.7-PSUFEGG7pO0gjWmlkXJl/dist=es2020,mode=imports,min/optimized/vega-force.js").unwrap();
//         println!("{}", source);
//     }
//
//     #[tokio::test]
//     async fn test_convert() {
//         convert().await.unwrap();
//     }
// }
