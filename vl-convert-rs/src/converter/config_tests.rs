use super::*;
use std::io::Write;

#[test]
fn test_empty_config() {
    let config: VlcConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config, VlcConfig::default());
}

#[test]
fn test_base_url_setting_default() {
    let config: VlcConfig = serde_json::from_str(r#"{"base_url": "default"}"#).unwrap();
    assert_eq!(config.base_url, BaseUrlSetting::Default);
}

#[test]
fn test_base_url_setting_disabled() {
    let config: VlcConfig = serde_json::from_str(r#"{"base_url": "disabled"}"#).unwrap();
    assert_eq!(config.base_url, BaseUrlSetting::Disabled);
}

#[test]
fn test_base_url_setting_custom() {
    let config: VlcConfig =
        serde_json::from_str(r#"{"base_url": "https://example.com/"}"#).unwrap();
    assert_eq!(
        config.base_url,
        BaseUrlSetting::Custom("https://example.com/".to_string())
    );
}

#[test]
fn test_missing_fonts_policy() {
    for (json, expected) in [
        ("\"fallback\"", MissingFontsPolicy::Fallback),
        ("\"warn\"", MissingFontsPolicy::Warn),
        ("\"error\"", MissingFontsPolicy::Error),
    ] {
        let policy: MissingFontsPolicy = serde_json::from_str(json).unwrap();
        assert_eq!(policy, expected);
    }
}

#[test]
fn test_format_locale_name() {
    let locale: FormatLocale = serde_json::from_str(r#""de-DE""#).unwrap();
    assert!(matches!(locale, FormatLocale::Name(ref s) if s == "de-DE"));
}

#[test]
fn test_format_locale_object() {
    let locale: FormatLocale =
        serde_json::from_str(r#"{"decimal": ",", "thousands": "."}"#).unwrap();
    assert!(matches!(locale, FormatLocale::Object(_)));
}

#[test]
fn test_google_font_request() {
    let req: GoogleFontRequest = serde_json::from_str(r#"{"family": "Roboto"}"#).unwrap();
    assert_eq!(req.family, "Roboto");
    assert!(req.variants.is_none());
}

#[test]
fn test_full_config() {
    let json = r##"{
        "num_workers": 2,
        "auto_google_fonts": true,
        "missing_fonts": "warn",
        "max_v8_heap_size_mb": 512,
        "default_theme": "dark",
        "themes": {
            "custom": {"background": "#333"}
        }
    }"##;
    let config: VlcConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.num_workers, 2);
    assert!(config.auto_google_fonts);
    assert_eq!(config.missing_fonts, MissingFontsPolicy::Warn);
    assert_eq!(config.max_v8_heap_size_mb, 512);
    assert_eq!(config.default_theme, Some("dark".to_string()));
    assert!(config.themes.is_some());
}

#[test]
fn test_jsonc_comments() {
    let jsonc = r#"{
        // This is a comment
        "auto_google_fonts": true,
        /* Block comment */
        "missing_fonts": "warn"
    }"#;
    let value: serde_json::Value = jsonc_parser::parse_to_serde_value(
        jsonc,
        &jsonc_parser::ParseOptions {
            allow_comments: true,
            allow_trailing_commas: true,
            allow_loose_object_property_names: false,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
        },
    )
    .unwrap();
    let config: VlcConfig = serde_json::from_value(value).unwrap();
    assert!(config.auto_google_fonts);
    assert_eq!(config.missing_fonts, MissingFontsPolicy::Warn);
}

#[test]
fn test_unknown_fields_ignored() {
    let json = r#"{"unknown_field": 42, "auto_google_fonts": true}"#;
    let config: VlcConfig = serde_json::from_str(json).unwrap();
    assert!(config.auto_google_fonts);
}

#[test]
fn test_from_file_url_base_url_not_rebased() {
    let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
    writeln!(
        config_file,
        r#"{{"base_url": "https://cdn.example.com/data/"}}"#
    )
    .unwrap();
    let config = VlcConfig::from_file(config_file.path()).unwrap();
    assert_eq!(
        config.base_url,
        BaseUrlSetting::Custom("https://cdn.example.com/data/".to_string())
    );
}

#[test]
fn test_from_file_url_plugin_not_rebased() {
    let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
    writeln!(
        config_file,
        r#"{{"vega_plugins": ["https://esm.sh/my-plugin@1.0"]}}"#
    )
    .unwrap();
    let config = VlcConfig::from_file(config_file.path()).unwrap();
    assert_eq!(
        config.vega_plugins.as_deref(),
        Some(&["https://esm.sh/my-plugin@1.0".to_string()][..])
    );
}

#[test]
fn test_from_file_inline_esm_plugin_not_rebased() {
    let inline = "export default function(vega) {}";
    let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
    writeln!(config_file, r#"{{"vega_plugins": ["{inline}"]}}"#).unwrap();
    let config = VlcConfig::from_file(config_file.path()).unwrap();
    assert_eq!(
        config.vega_plugins.as_deref(),
        Some(&[inline.to_string()][..])
    );
}

#[test]
fn test_from_file_relative_plugin_rebased() {
    let mut config_file = tempfile::NamedTempFile::with_suffix(".jsonc").unwrap();
    writeln!(config_file, r#"{{"vega_plugins": ["./my-plugin.js"]}}"#).unwrap();
    let config = VlcConfig::from_file(config_file.path()).unwrap();
    let expected = config_file
        .path()
        .parent()
        .unwrap()
        .join("./my-plugin.js")
        .to_string_lossy()
        .to_string();
    assert_eq!(config.vega_plugins.as_deref(), Some(&[expected][..]));
}
