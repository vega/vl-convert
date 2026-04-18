use serde::ser::{SerializeMap, Serializer};
use std::fmt;
use std::time::Duration;
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::registry::LookupSpan;

use tracing_log::NormalizeEvent;

/// Custom `FormatEvent` that produces flat JSON with standard attribute names
/// compatible with Datadog, Grafana, Elastic, and other observability platforms.
pub struct FlatJsonFormatter;

/// Key remapping for span fields to standard HTTP/trace attribute names.
fn remap_span_key(key: &str) -> Option<&'static str> {
    match key {
        "method" => Some("http.method"),
        "uri" => Some("http.url"),
        "version" => Some("http.version"),
        "user_agent" => Some("http.useragent"),
        "request_id" => Some("http.request_id"),
        "trace_id" => Some("trace_id"),
        "span_id" => Some("span_id"),
        "budget_outcome" => Some("budget.outcome"),
        "budget_charged_ms" => Some("budget.charged_ms"),
        "budget_global_remaining_ms" => Some("budget.global_remaining_ms"),
        "budget_ip_remaining_ms" => Some("budget.ip_remaining_ms"),
        "budget_client_ip" => Some("budget.client_ip"),
        _ => None,
    }
}

impl<S, N> FormatEvent<S, N> for FlatJsonFormatter
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
        let normalized = event.normalized_metadata();
        let meta = normalized.as_ref().unwrap_or_else(|| event.metadata());

        let mut buf = Vec::new();
        {
            let mut ser = serde_json::Serializer::new(&mut buf);
            let mut map = ser.serialize_map(None).map_err(|_| fmt::Error)?;

            map.serialize_entry("timestamp", &chrono::Utc::now().to_rfc3339())
                .map_err(|_| fmt::Error)?;
            map.serialize_entry("level", &meta.level().as_str().to_ascii_lowercase())
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
                                    if v.as_str() == Some("") {
                                        continue;
                                    }
                                    let key = remap_span_key(k).unwrap_or(k.as_str());
                                    map.serialize_entry(key, v).map_err(|_| fmt::Error)?;
                                }
                            }
                        }
                    }
                }
            }

            // Flatten event fields
            let mut visitor = FlatJsonEventVisitor::new();
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

struct FlatJsonEventVisitor {
    message: Option<String>,
    fields: Vec<(String, serde_json::Value)>,
}

impl FlatJsonEventVisitor {
    fn new() -> Self {
        Self {
            message: None,
            fields: Vec::new(),
        }
    }
}

impl tracing::field::Visit for FlatJsonEventVisitor {
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
pub struct FlatJsonOnResponse;

impl<B> tower_http::trace::OnResponse<B> for FlatJsonOnResponse {
    fn on_response(self, response: &http::Response<B>, latency: Duration, _span: &tracing::Span) {
        let status = response.status().as_u16();
        let duration_ns = latency.as_nanos() as i64;
        let response_time_ms = latency.as_secs_f64() * 1000.0;
        tracing::info!(
            http.status_code = status,
            duration = duration_ns,
            response_time_ms = response_time_ms,
            "response"
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{capture_json_subscriber, BufferWriter};

    #[test]
    fn test_json_level_is_lowercase() {
        let buf = BufferWriter::default();
        let subscriber = capture_json_subscriber(buf.clone());
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hi");
        });

        let output = buf.snapshot();
        let event: serde_json::Value = output
            .lines()
            .find_map(|l| serde_json::from_str(l).ok())
            .expect("one event captured");
        assert_eq!(
            event["level"], "info",
            "level should be lowercase (Railway convention). captured: {output}"
        );
    }
}
