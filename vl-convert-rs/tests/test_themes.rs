use vl_convert_rs::VlConverter;

#[tokio::test]
async fn test_get_themes_dark_background() {
    // Create Vega-Lite Converter and perform conversion
    let mut converter = VlConverter::new();
    if let serde_json::Value::Object(all_themes) = converter.get_themes().await.unwrap() {
        if let serde_json::Value::Object(dark) = all_themes.get("dark").unwrap() {
            let background = dark.get("background").unwrap().as_str().unwrap();
            assert_eq!(background, "#333");
        } else {
            panic!("Expected dark theme to be an object")
        }
    } else {
        panic!("Expected themes to be an object")
    }
}
