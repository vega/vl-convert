//! Text utilities for JavaScript bundling.
//!
//! Provides helper functions for processing JavaScript source text,
//! including BOM stripping and JSON-to-JavaScript transformation.

/// Strips the UTF-8 BOM (byte order mark) from the beginning of text if present.
///
/// The BOM is U+FEFF (0xEF 0xBB 0xBF in UTF-8) and is sometimes present at the
/// start of files.
pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}

/// Transforms JSON source code into JavaScript that exports the parsed JSON.
///
/// This allows JSON files to be imported as ES modules by wrapping the JSON
/// content in `export default JSON.parse("...")`.
///
/// # Example
/// ```ignore
/// // Input: {"key": "value"}
/// // Output: export default JSON.parse("{\"key\": \"value\"}")
/// ```
pub fn transform_json_source(source: &str) -> String {
    // Escape the JSON string for embedding in a JavaScript string literal
    let escaped = escape_json_string(source);
    format!("export default JSON.parse(\"{}\")", escaped)
}

/// Escapes a string for safe embedding in a JavaScript string literal.
///
/// Handles special characters like quotes, backslashes, and control characters.
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            // Control characters (U+0000 to U+001F)
            c if c < '\x20' => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_bom_with_bom() {
        let with_bom = "\u{FEFF}hello";
        assert_eq!(strip_bom(with_bom), "hello");
    }

    #[test]
    fn test_strip_bom_without_bom() {
        let without_bom = "hello";
        assert_eq!(strip_bom(without_bom), "hello");
    }

    #[test]
    fn test_transform_json_source() {
        let json = r#"{"key": "value"}"#;
        let result = transform_json_source(json);
        assert!(result.starts_with("export default JSON.parse(\""));
        assert!(result.contains("\\\"key\\\""));
    }

    #[test]
    fn test_escape_json_string() {
        assert_eq!(escape_json_string("hello"), "hello");
        assert_eq!(escape_json_string("\"quoted\""), "\\\"quoted\\\"");
        assert_eq!(escape_json_string("line\nbreak"), "line\\nbreak");
    }
}
