use std::net::SocketAddr;

use once_cell::sync::Lazy;
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{LogFormat, ServeConfig};

pub struct TestServer {
    pub base_url: String,
    pub client: reqwest::Client,
}

pub fn default_serve_config() -> ServeConfig {
    ServeConfig {
        host: "127.0.0.1".to_string(),
        port: 0,
        api_key: None,
        cors_origin: None,
        max_concurrent_requests: None,
        request_timeout_secs: 30,
        drain_timeout_secs: 30,
        max_body_size_mb: 50,
        opaque_errors: false,
        require_user_agent: false,
        log_format: LogFormat::Text,
        per_ip_budget_ms: None,
        global_budget_ms: None,
        budget_estimate_ms: 2000,
        admin_port: None,
        trust_proxy: false,
    }
}

pub fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

pub fn start_server_sync(config: VlcConfig, serve_config: ServeConfig) -> TestServer {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let (app, _converter) = vl_convert_server::build_app(config, &serve_config).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            tx.send(port).unwrap();
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .ok();
        });
    });

    let port = rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
    let base_url = format!("http://127.0.0.1:{port}");

    // Poll for readiness using raw TCP connect (avoids reqwest::blocking inside
    // an async context, which panics due to nested runtimes).
    for _ in 0..150 {
        if std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    TestServer {
        base_url,
        client: reqwest::Client::new(),
    }
}

pub static DEFAULT_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let serve_config = default_serve_config();
    start_server_sync(config, serve_config)
});

pub static AUTH_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.api_key = Some("test-secret".to_string());
    start_server_sync(config, serve_config)
});

pub static UA_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.require_user_agent = true;
    start_server_sync(config, serve_config)
});

pub static OPAQUE_SERVER: Lazy<TestServer> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.opaque_errors = true;
    start_server_sync(config, serve_config)
});

pub fn start_budget_server(
    per_ip_ms: Option<i64>,
    global_ms: Option<i64>,
    estimate_ms: i64,
) -> (TestServer, u16) {
    let config = VlcConfig::default();
    let admin_port = find_free_port();
    let mut serve_config = default_serve_config();
    serve_config.per_ip_budget_ms = per_ip_ms;
    serve_config.global_budget_ms = global_ms;
    serve_config.budget_estimate_ms = estimate_ms;
    serve_config.admin_port = Some(admin_port);
    let server = start_server_sync(config, serve_config);
    (server, admin_port)
}

pub fn simple_vl_spec() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": 1, "b": 2}]},
        "mark": "bar",
        "encoding": {"x": {"field": "a"}, "y": {"field": "b"}}
    })
}

pub fn simple_vg_spec() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 100, "height": 100,
        "data": [{"name": "values", "values": [{"x": 0, "y": 0}, {"x": 1, "y": 1}]}],
        "marks": [{"type": "rect", "from": {"data": "values"},
            "encode": {"enter": {
                "x": {"field": "x", "type": "quantitative"},
                "y": {"field": "y", "type": "quantitative"},
                "width": {"value": 10}, "height": {"value": 10},
                "fill": {"value": "steelblue"}
            }}}]
    })
}

pub fn simple_svg() -> &'static str {
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="red"/></svg>"#
}
