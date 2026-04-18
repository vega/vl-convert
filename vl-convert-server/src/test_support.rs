use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower::Service;

use crate::budget::{self, BudgetTracker};
use crate::{build_middleware_stack, json_fmt, LogFormat, ServeConfig};

#[derive(Clone, Default)]
pub(crate) struct BufferWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl BufferWriter {
    pub(crate) fn snapshot(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).to_string()
    }
}

impl std::io::Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriter {
    type Writer = BufferWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

pub(crate) fn capture_json_subscriber(
    buf: BufferWriter,
) -> impl tracing::Subscriber + Send + Sync + 'static {
    tracing_subscriber::fmt()
        .event_format(json_fmt::FlatJsonFormatter)
        .fmt_fields(tracing_subscriber::fmt::format::JsonFields::new())
        .with_writer(buf)
        .with_max_level(tracing::Level::INFO)
        .finish()
}

pub(crate) fn find_response_event(buf: &BufferWriter) -> serde_json::Value {
    let output = buf.snapshot();
    let events: Vec<serde_json::Value> = output
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    events
        .into_iter()
        .find(|e| e.get("message").and_then(|m| m.as_str()) == Some("response"))
        .expect("no response event captured")
}

pub(crate) fn default_serve_config() -> ServeConfig {
    ServeConfig {
        port: 0,
        ..ServeConfig::default()
    }
}

pub(crate) fn run_budget_request(
    tracker: Arc<BudgetTracker>,
    serve_config_mutator: impl FnOnce(&mut ServeConfig),
    uri: &str,
) -> (BufferWriter, axum::http::Response<axum::body::Body>) {
    async fn ok_handler() -> &'static str {
        "ok"
    }

    let router = Router::new()
        .route("/t", get(ok_handler))
        .layer(axum::middleware::from_fn(
            move |req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| {
                let tracker = tracker.clone();
                async move { budget::middleware(tracker, false, false, req, next).await }
            },
        ));

    let mut serve_config = default_serve_config();
    serve_config.log_format = LogFormat::Json;
    serve_config_mutator(&mut serve_config);

    let mut app = build_middleware_stack(router, &serve_config);

    let buf = BufferWriter::default();
    let subscriber = capture_json_subscriber(buf.clone());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let response = tracing::subscriber::with_default(subscriber, || {
        rt.block_on(async move {
            Service::call(
                &mut app,
                axum::http::Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
        })
    });

    (buf, response)
}
