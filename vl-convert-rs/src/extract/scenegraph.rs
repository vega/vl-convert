use super::types::{parse_css_font_family, FontFamilyEntry, FontKey};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

/// Walk a rendered Vega scenegraph and extract unique characters per
/// (font-family, weight, style).
///
/// The scenegraph must be the `"scenegraph"` value returned by
/// `vegaToScenegraph` -- the root mark node with `marktype`, `items`, etc.
pub fn extract_text_by_font(scenegraph: &Value) -> HashMap<FontKey, BTreeSet<char>> {
    let mut result: HashMap<FontKey, BTreeSet<char>> = HashMap::new();
    walk_scenegraph_mark(scenegraph, &mut result);
    result
}

fn normalize_weight(v: Option<&Value>) -> String {
    match v {
        None => "400".to_string(),
        Some(Value::String(s)) => match s.as_str() {
            "normal" => "400".to_string(),
            "bold" => "700".to_string(),
            "bolder" => "700".to_string(),
            "lighter" => "100".to_string(),
            other => other
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n > 0.0)
                .map(|n| format!("{}", n as i32))
                .unwrap_or_else(|| "400".to_string()),
        },
        Some(Value::Number(n)) => n
            .as_f64()
            .filter(|f| f.is_finite() && *f > 0.0)
            .map(|f| format!("{}", f as i32))
            .unwrap_or_else(|| "400".to_string()),
        _ => "400".to_string(),
    }
}

fn normalize_style(v: Option<&Value>) -> String {
    match v {
        None => "normal".to_string(),
        Some(Value::String(s)) => {
            let lower = s.trim().to_lowercase();
            if lower == "italic" || lower == "oblique" {
                "italic".to_string()
            } else {
                "normal".to_string()
            }
        }
        _ => "normal".to_string(),
    }
}

/// Convert a scenegraph text item's `text` field to a `String`.
///
/// Mirrors the JS `String(text)` coercion: scalars are stringified directly,
/// arrays are joined with commas (Vega uses arrays for multiline text).
fn text_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Array(arr) => {
            let joined: String = arr
                .iter()
                .map(|elem| match elem {
                    Value::String(s) => s.as_str().to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(",");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        _ => None,
    }
}

fn walk_scenegraph_mark(node: &Value, result: &mut HashMap<FontKey, BTreeSet<char>>) {
    let marktype = node.get("marktype").and_then(|v| v.as_str());

    match marktype {
        Some("text") => {
            if let Some(items) = node.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    let text = match item.get("text").and_then(text_to_string) {
                        Some(s) => s,
                        None => continue,
                    };

                    let font_str = item
                        .get("font")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sans-serif")
                        .trim();

                    let families = parse_css_font_family(font_str);
                    let family = match families.first() {
                        Some(FontFamilyEntry::Named(name)) => name.clone(),
                        _ => continue,
                    };

                    let weight = normalize_weight(item.get("fontWeight"));
                    let style = normalize_style(item.get("fontStyle"));

                    let key = FontKey {
                        family,
                        weight,
                        style,
                    };
                    let chars = result.entry(key).or_default();
                    for ch in text.chars() {
                        chars.insert(ch);
                    }
                }
            }
        }
        Some("group") => {
            if let Some(items) = node.get("items").and_then(|v| v.as_array()) {
                for group_item in items {
                    if let Some(child_marks) = group_item.get("items").and_then(|v| v.as_array()) {
                        for child_mark in child_marks {
                            walk_scenegraph_mark(child_mark, result);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
