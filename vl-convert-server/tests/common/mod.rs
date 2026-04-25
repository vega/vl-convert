#![allow(dead_code)]

use once_cell::sync::Lazy;
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{BoundListener, ListenAddr, ServeConfig};

pub struct ServerHandle {
    pub base_url: String,
    pub client: reqwest::Client,
    /// Kept alive for the server's lifetime. UDS servers need this so
    /// the tempdir containing the socket file isn't dropped while the
    /// server is running.
    pub _tempdir: Option<tempfile::TempDir>,
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
    // Bind the main listener synchronously on the test thread. Port 0
    // → kernel picks a free port; holding the listener across the
    // move into the background thread keeps the port reserved (no
    // TOCTOU). The kernel starts queueing incoming SYNs into the
    // listen backlog immediately, so a main-port readiness probe
    // isn't needed.
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    std_listener.set_nonblocking(true).unwrap();
    let addr = std_listener.local_addr().unwrap();

    // Ready signal: admin listener (when enabled) is bound inside
    // build_app, which runs on the spawned thread's runtime. The test
    // thread blocks on this signal so ServerHandle is only published
    // after both listeners are accepting.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let built = vl_convert_server::build_app(config, &serve_config)
                .await
                .expect("build_app failed");
            let listener =
                BoundListener::Tcp(tokio::net::TcpListener::from_std(std_listener).unwrap());
            ready_tx.send(()).ok();
            // Tests don't trigger graceful shutdown; the server runs
            // until the background thread's runtime is dropped.
            vl_convert_server::serve(listener, built, std::future::pending())
                .await
                .ok();
        });
    });

    ready_rx
        .recv()
        .expect("server thread exited before signaling ready");

    ServerHandle {
        base_url: format!("http://{addr}"),
        client: reqwest::Client::new(),
        _tempdir: None,
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

/// Result of `start_budget_server`. The admin URL is a full base URL
/// string (`http://host:port`, or `unix:///path` if a UDS variant is
/// ever added) so callers can construct admin endpoints without
/// knowing the transport.
pub struct BudgetServer {
    pub handle: ServerHandle,
    pub admin_base_url: String,
    /// Kept alive when admin is UDS so the tempdir holding the admin
    /// socket file isn't dropped while the test runs.
    pub _admin_tempdir: Option<tempfile::TempDir>,
}

pub fn start_budget_server(
    per_ip_ms: Option<i64>,
    global_ms: Option<i64>,
    hold_ms: i64,
    trust_proxy: bool,
) -> BudgetServer {
    let config = VlcConfig::default();
    let admin_port = find_free_port();
    let mut serve_config = default_serve_config();
    serve_config.per_ip_budget_ms = per_ip_ms;
    serve_config.global_budget_ms = global_ms;
    serve_config.budget_hold_ms = hold_ms;
    serve_config.admin = Some(ListenAddr::Tcp {
        host: "127.0.0.1".to_string(),
        port: admin_port,
    });
    serve_config.trust_proxy = trust_proxy;
    BudgetServer {
        handle: start_server_sync(config, serve_config),
        admin_base_url: format!("http://127.0.0.1:{admin_port}"),
        _admin_tempdir: None,
    }
}

/// Start a server suitable for admin-config integration tests. Picks a free
/// TCP port for the admin listener, clones `serve_config`, and wires the
/// admin listener at `127.0.0.1:<port>`. Reuses the `BudgetServer` struct
/// because the shape (main handle + admin URL) is identical — admin-config
/// tests aren't budget-specific.
pub fn start_admin_config_server(
    config: VlcConfig,
    mut serve_config: ServeConfig,
) -> BudgetServer {
    let admin_port = find_free_port();
    serve_config.admin = Some(ListenAddr::Tcp {
        host: "127.0.0.1".to_string(),
        port: admin_port,
    });
    BudgetServer {
        handle: start_server_sync(config, serve_config),
        admin_base_url: format!("http://127.0.0.1:{admin_port}"),
        _admin_tempdir: None,
    }
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

// ============================================================================
// UDS test infrastructure
// ============================================================================

/// Start a server whose main + optional admin listeners bind to UDS
/// sockets in a per-test tempdir. The returned `ServerHandle.base_url`
/// is a `unix:///absolute/path` URL; use [`uds_get`] / [`uds_post_json`]
/// to issue HTTP requests against it (the workspace-pinned reqwest
/// has no UDS transport, so we drive raw hyper + UnixStream directly).
///
/// Both listeners share one tempdir so the server stays alive as long
/// as the tempdir does.
#[cfg(unix)]
pub fn start_uds_server_sync(
    mut serve_config: ServeConfig,
    main_sock_name: &str,
    admin_sock_name: Option<&str>,
) -> ServerHandle {
    let tmp = tempfile::tempdir().expect("mktempdir failed");
    let main_path = tmp.path().join(main_sock_name);
    serve_config.main = ListenAddr::Uds {
        path: main_path.clone(),
    };
    if let Some(name) = admin_sock_name {
        serve_config.admin = Some(ListenAddr::Uds {
            path: tmp.path().join(name),
        });
    }

    // Spawn server in a background thread with its own current-thread
    // runtime. Same pattern as `start_server_sync` but targets UDS.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let config = VlcConfig::default();
    let sc = serve_config.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let built = vl_convert_server::build_app(config, &sc)
                .await
                .expect("build_app failed");
            let listener = vl_convert_server::bind_listener(&sc.main, sc.socket_mode)
                .await
                .expect("bind_listener main failed");
            ready_tx.send(()).ok();
            vl_convert_server::serve(listener, built, std::future::pending())
                .await
                .ok();
        });
    });

    ready_rx
        .recv()
        .expect("server thread exited before signaling ready");

    ServerHandle {
        base_url: format!("unix://{}", main_path.display()),
        client: reqwest::Client::new(), // unused for UDS — use uds_* helpers
        _tempdir: Some(tmp),
    }
}

/// UDS HTTP GET helper. Returns `(status, body_bytes, headers)`.
///
/// reqwest has no UDS transport at workspace pin 0.11 — this uses raw
/// hyper + hyper-util + `tokio::net::UnixStream` instead.
#[cfg(unix)]
pub async fn uds_get(
    sock_path: &std::path::Path,
    path: &str,
) -> (
    hyper::StatusCode,
    bytes::Bytes,
    hyper::HeaderMap<hyper::header::HeaderValue>,
) {
    uds_request(sock_path, hyper::Method::GET, path, None, &[]).await
}

/// UDS HTTP POST-JSON helper.
#[cfg(unix)]
pub async fn uds_post_json(
    sock_path: &std::path::Path,
    path: &str,
    body: &serde_json::Value,
) -> (
    hyper::StatusCode,
    bytes::Bytes,
    hyper::HeaderMap<hyper::header::HeaderValue>,
) {
    let body_bytes = serde_json::to_vec(body).unwrap();
    uds_request(
        sock_path,
        hyper::Method::POST,
        path,
        Some(body_bytes),
        &[(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        )],
    )
    .await
}

/// Build and drive a single HTTP/1 request over a UDS connection.
#[cfg(unix)]
pub async fn uds_request(
    sock_path: &std::path::Path,
    method: hyper::Method,
    path: &str,
    body: Option<Vec<u8>>,
    extra_headers: &[(hyper::header::HeaderName, hyper::header::HeaderValue)],
) -> (
    hyper::StatusCode,
    bytes::Bytes,
    hyper::HeaderMap<hyper::header::HeaderValue>,
) {
    use http_body_util::{BodyExt, Full};
    use hyper_util::rt::TokioIo;
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(sock_path)
        .await
        .expect("UnixStream::connect failed");
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .expect("hyper handshake failed");
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let mut req = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header(hyper::header::HOST, "localhost");
    for (k, v) in extra_headers {
        req = req.header(k, v);
    }
    let req = req
        .body(Full::<bytes::Bytes>::from(body.unwrap_or_default()))
        .expect("request build failed");

    let resp = sender.send_request(req).await.expect("send_request failed");
    let (parts, incoming) = resp.into_parts();
    let body = incoming
        .collect()
        .await
        .expect("collect body failed")
        .to_bytes();
    (parts.status, body, parts.headers)
}
