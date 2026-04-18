#![allow(dead_code)]

use once_cell::sync::Lazy;
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::ServeConfig;

pub struct ServerHandle {
    pub base_url: String,
    pub client: reqwest::Client,
}

pub fn default_serve_config() -> ServeConfig {
    ServeConfig {
        budget_hold_ms: 2000,
        ..ServeConfig::default()
    }
}

pub fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

pub fn start_server_sync(config: VlcConfig, serve_config: ServeConfig) -> ServerHandle {
    // Bind the listener synchronously on the test thread. Port 0 → kernel
    // picks a free port; holding the listener across the move into the
    // background thread keeps the port reserved (no TOCTOU). The kernel
    // starts queueing incoming SYNs into the listen backlog immediately,
    // so no readiness probe is needed.
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let addr = std_listener.local_addr().unwrap();

    let built = vl_convert_server::build_app(config, &serve_config).unwrap();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(std_listener).unwrap();
            // Tests don't trigger graceful shutdown; the server runs
            // until the background thread's runtime is dropped.
            vl_convert_server::serve(listener, built, std::future::pending())
                .await
                .ok();
        });
    });

    ServerHandle {
        base_url: format!("http://{addr}"),
        client: reqwest::Client::new(),
    }
}

pub static DEFAULT_SERVER: Lazy<ServerHandle> = Lazy::new(|| {
    let config = VlcConfig::default();
    let serve_config = default_serve_config();
    start_server_sync(config, serve_config)
});

pub static AUTH_SERVER: Lazy<ServerHandle> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.api_key = Some("test-secret".to_string());
    start_server_sync(config, serve_config)
});

pub static UA_SERVER: Lazy<ServerHandle> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.require_user_agent = true;
    start_server_sync(config, serve_config)
});

pub static OPAQUE_SERVER: Lazy<ServerHandle> = Lazy::new(|| {
    let config = VlcConfig::default();
    let mut serve_config = default_serve_config();
    serve_config.opaque_errors = true;
    start_server_sync(config, serve_config)
});

pub fn start_budget_server(
    per_ip_ms: Option<i64>,
    global_ms: Option<i64>,
    hold_ms: i64,
    trust_proxy: bool,
) -> (ServerHandle, u16) {
    let config = VlcConfig::default();
    let admin_port = find_free_port();
    let mut serve_config = default_serve_config();
    serve_config.per_ip_budget_ms = per_ip_ms;
    serve_config.global_budget_ms = global_ms;
    serve_config.budget_hold_ms = hold_ms;
    serve_config.admin_port = Some(admin_port);
    serve_config.trust_proxy = trust_proxy;
    let server = start_server_sync(config, serve_config);

    for _ in 0..150 {
        if std::net::TcpStream::connect(format!("127.0.0.1:{admin_port}")).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

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
