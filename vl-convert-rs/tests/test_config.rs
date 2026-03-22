use serde_json::json;
use std::collections::HashMap;
use vl_convert_rs::converter::{SvgOpts, VlConverterConfig, VlOpts};
use vl_convert_rs::VlConverter;

fn simple_vl_bar_spec() -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": "A", "b": 28}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    })
}

#[tokio::test]
async fn test_custom_theme_applied() {
    let mut themes = HashMap::new();
    themes.insert("mytest".to_string(), json!({"background": "#abcdef"}));

    let converter = VlConverter::with_config(VlConverterConfig {
        themes: Some(themes),
        default_theme: Some("mytest".to_string()),
        ..Default::default()
    })
    .unwrap();

    let svg = converter
        .vegalite_to_svg(simple_vl_bar_spec(), VlOpts::default(), SvgOpts::default())
        .await
        .unwrap();

    assert!(
        svg.contains("#abcdef"),
        "SVG should contain the custom theme background color. Got: {}",
        &svg[..svg.len().min(500)]
    );
}

#[tokio::test]
async fn test_per_request_theme_overrides_default() {
    let mut themes = HashMap::new();
    themes.insert("mytest".to_string(), json!({"background": "#abcdef"}));

    let converter = VlConverter::with_config(VlConverterConfig {
        themes: Some(themes),
        default_theme: Some("mytest".to_string()),
        ..Default::default()
    })
    .unwrap();

    // Per-request "dark" theme should override the default "mytest"
    let svg = converter
        .vegalite_to_svg(
            simple_vl_bar_spec(),
            VlOpts {
                theme: Some("dark".to_string()),
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap();

    // Should NOT have the custom background
    assert!(
        !svg.contains("#abcdef"),
        "Per-request theme should override the default custom theme"
    );
    // Should have the dark theme's background
    assert!(
        svg.contains("#333"),
        "SVG should contain the dark theme background"
    );
}

#[tokio::test]
async fn test_custom_theme_visible_in_get_themes() {
    let mut themes = HashMap::new();
    themes.insert("corporate".to_string(), json!({"background": "#f5f5f5"}));

    let converter = VlConverter::with_config(VlConverterConfig {
        themes: Some(themes),
        ..Default::default()
    })
    .unwrap();

    let all_themes = converter.get_themes().await.unwrap();
    let themes_obj = all_themes.as_object().unwrap();

    assert!(
        themes_obj.contains_key("corporate"),
        "Custom theme should appear in get_themes(). Keys: {:?}",
        themes_obj.keys().collect::<Vec<_>>()
    );

    // Built-in themes should still be present
    assert!(
        themes_obj.contains_key("dark"),
        "Built-in themes should still be present"
    );

    // Verify the custom theme config
    let corp = &themes_obj["corporate"];
    assert_eq!(corp["background"], "#f5f5f5");
}

#[tokio::test]
async fn test_default_theme_without_custom_themes() {
    // Using a built-in theme as default (no custom themes needed)
    let converter = VlConverter::with_config(VlConverterConfig {
        default_theme: Some("dark".to_string()),
        ..Default::default()
    })
    .unwrap();

    let svg = converter
        .vegalite_to_svg(simple_vl_bar_spec(), VlOpts::default(), SvgOpts::default())
        .await
        .unwrap();

    assert!(
        svg.contains("#333"),
        "SVG should contain the dark theme background"
    );
}

#[tokio::test]
async fn test_default_locale_applied() {
    let converter = VlConverter::with_config(VlConverterConfig {
        default_format_locale: Some(vl_convert_rs::converter::FormatLocale::Name(
            "fr-FR".to_string(),
        )),
        ..Default::default()
    })
    .unwrap();

    // Spec with a formatted number that differs between en-US and fr-FR
    let spec = json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": "A", "b": 1234.5}]},
        "mark": "text",
        "encoding": {
            "text": {"field": "b", "type": "quantitative", "format": ",.1f"}
        }
    });

    let svg = converter
        .vegalite_to_svg(spec, VlOpts::default(), SvgOpts::default())
        .await
        .unwrap();

    // French locale uses non-breaking space as thousands separator and comma as decimal
    // The exact rendering depends on Vega's formatting, but it should differ from "1,234.5"
    assert!(
        !svg.contains("1,234.5"),
        "With fr-FR locale, should not use English number formatting. Got: {}",
        &svg[..svg.len().min(500)]
    );
}

#[tokio::test]
async fn test_per_request_locale_overrides_default() {
    let converter = VlConverter::with_config(VlConverterConfig {
        default_format_locale: Some(vl_convert_rs::converter::FormatLocale::Name(
            "fr-FR".to_string(),
        )),
        ..Default::default()
    })
    .unwrap();

    let spec = json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": "A", "b": 1234.5}]},
        "mark": "text",
        "encoding": {
            "text": {"field": "b", "type": "quantitative", "format": ",.1f"}
        }
    });

    // Per-request en-US should override the default fr-FR
    let svg = converter
        .vegalite_to_svg(
            spec,
            VlOpts {
                format_locale: Some(vl_convert_rs::converter::FormatLocale::Name(
                    "en-US".to_string(),
                )),
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap();

    assert!(
        svg.contains("1,234.5"),
        "Per-request en-US locale should produce English formatting. Got: {}",
        &svg[..svg.len().min(500)]
    );
}
