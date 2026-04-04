use deno_core::error::AnyError;

/// A JSON value that may already be serialized to a string.
/// When the caller already has a JSON string (e.g. from Python), this avoids
/// a redundant parse->Value->serialize round-trip.
#[derive(Debug, Clone)]
pub enum ValueOrString {
    /// Pre-serialized JSON string -- stored directly, no serialization needed
    JsonString(String),
    /// Parsed serde_json::Value -- will be serialized to JSON when needed
    Value(serde_json::Value),
}

impl From<serde_json::Value> for ValueOrString {
    fn from(v: serde_json::Value) -> Self {
        ValueOrString::Value(v)
    }
}

impl From<&serde_json::Value> for ValueOrString {
    fn from(v: &serde_json::Value) -> Self {
        ValueOrString::Value(v.clone())
    }
}

impl From<String> for ValueOrString {
    fn from(s: String) -> Self {
        ValueOrString::JsonString(s)
    }
}

impl ValueOrString {
    /// Convert to a serde_json::Value, parsing if necessary.
    pub fn to_value(self) -> Result<serde_json::Value, AnyError> {
        match self {
            ValueOrString::Value(v) => Ok(v),
            ValueOrString::JsonString(s) => Ok(serde_json::from_str(&s)?),
        }
    }
}

/// Apply background, width, and height overrides to a spec.
/// Works for both Vega and Vega-Lite specs.
pub(crate) fn apply_spec_overrides(
    spec: ValueOrString,
    background: &Option<String>,
    width: Option<f32>,
    height: Option<f32>,
) -> Result<ValueOrString, AnyError> {
    if background.is_none() && width.is_none() && height.is_none() {
        return Ok(spec);
    }
    let mut val = spec.to_value()?;
    if let Some(obj) = val.as_object_mut() {
        if let Some(bg) = background {
            obj.insert(
                "background".to_string(),
                serde_json::Value::String(bg.clone()),
            );
        }
        if let Some(w) = width {
            obj.insert(
                "width".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(w as f64).unwrap_or(serde_json::Number::from(0)),
                ),
            );
        }
        if let Some(h) = height {
            obj.insert(
                "height".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(h as f64).unwrap_or(serde_json::Number::from(0)),
                ),
            );
        }
    }
    Ok(ValueOrString::Value(val))
}
