use serde::ser::{SerializeMap, Serializer};
use std::fmt;
use std::time::Duration;
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

// NormalizeEvent provides normalized_metadata() for correct handling of log crate events
use tracing_log::NormalizeEvent;

/// Custom `FormatEvent` that produces flat JSON with Datadog standard attribute names.
pub struct DatadogFormatter;

/// Key remapping for span fields (tower-http's request span).
fn remap_span_key(key: &str) -> Option<&'static str> {
    match key {
        "method" => Some("http.method"),
        "uri" => Some("http.url"),
        "version" => Some("http.version"),
        _ => None,
    }
}

impl<S, N> FormatEvent<S, N> for DatadogFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        // Use normalized_metadata for correct handling of log crate events
        let normalized = event.normalized_metadata();
        let meta = normalized.as_ref().unwrap_or_else(|| event.metadata());

        let mut buf = Vec::new();
        {
            let mut ser = serde_json::Serializer::new(&mut buf);
            let mut map = ser.serialize_map(None).map_err(|_| fmt::Error)?;

            // Fixed fields
            map.serialize_entry("timestamp", &chrono::Utc::now().to_rfc3339())
                .map_err(|_| fmt::Error)?;
            map.serialize_entry("level", &format!("{}", meta.level()))
                .map_err(|_| fmt::Error)?;
            map.serialize_entry("target", meta.target())
                .map_err(|_| fmt::Error)?;

            // Flatten span fields with key remapping
            if let Some(scope) = ctx.event_scope() {
                for span in scope.from_root() {
                    let ext = span.extensions();
                    if let Some(fields) = ext.get::<FormattedFields<N>>() {
                        let field_str = fields.to_string();
                        if !field_str.is_empty() {
                            if let Ok(obj) = serde_json::from_str::<
                                serde_json::Map<String, serde_json::Value>,
                            >(&field_str)
                            {
                                for (k, v) in &obj {
                                    let key = remap_span_key(k).unwrap_or(k.as_str());
                                    map.serialize_entry(key, v).map_err(|_| fmt::Error)?;
                                }
                            }
                        }
                    }
                }
            }

            // Flatten event fields
            let mut visitor = DatadogEventVisitor::new();
            event.record(&mut visitor);

            if let Some(msg) = &visitor.message {
                map.serialize_entry("message", msg)
                    .map_err(|_| fmt::Error)?;
            }
            for (k, v) in &visitor.fields {
                map.serialize_entry(k.as_str(), v).map_err(|_| fmt::Error)?;
            }

            map.end().map_err(|_| fmt::Error)?;
        }

        writer.write_str(std::str::from_utf8(&buf).map_err(|_| fmt::Error)?)?;
        writeln!(writer)
    }
}

/// Collects event fields, remapping keys to Datadog conventions.
struct DatadogEventVisitor {
    message: Option<String>,
    fields: Vec<(String, serde_json::Value)>,
}

impl DatadogEventVisitor {
    fn new() -> Self {
        Self {
            message: None,
            fields: Vec::new(),
        }
    }
}

impl tracing::field::Visit for DatadogEventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        let name = field.name();
        if name == "message" {
            self.message = Some(format!("{:?}", value));
        } else {
            self.fields.push((
                name.to_string(),
                serde_json::Value::String(format!("{:?}", value)),
            ));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push((
                name.to_string(),
                serde_json::Value::String(value.to_string()),
            ));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.push((
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        ));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.push((
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        ));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if let Some(n) = serde_json::Number::from_f64(value) {
            self.fields
                .push((field.name().to_string(), serde_json::Value::Number(n)));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .push((field.name().to_string(), serde_json::Value::Bool(value)));
    }
}

/// Custom `OnResponse` that records HTTP status and duration as properly-typed fields.
#[derive(Clone)]
pub struct DatadogOnResponse;

impl<B> tower_http::trace::OnResponse<B> for DatadogOnResponse {
    fn on_response(self, response: &http::Response<B>, latency: Duration, _span: &tracing::Span) {
        let status = response.status().as_u16();
        let duration_ns = latency.as_nanos() as i64;
        tracing::info!(
            http.status_code = status,
            duration = duration_ns,
            "response"
        );
    }
}
