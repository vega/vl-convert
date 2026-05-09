# vl-convert-server

Library crate for serving Vega-Lite and Vega conversions over HTTP. It wraps
`vl-convert-rs` with an axum-based REST API for SVG, PNG, PDF, JPEG, HTML,
scenegraph, theme, font, and JavaScript-bundling operations.

This crate does not publish a `vl-convert-server` binary. The `vl-convert`
package provides `vl-convert serve` as the reference binary built on this
crate. This README is library-first; CLI-specific behavior is called out
explicitly.

## Library Usage

```rust
use vl_convert_rs::converter::VlcConfig;
use vl_convert_server::{bind_listener, build_app, serve, ListenAddr, ServeConfig};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let serve_config = ServeConfig {
    main: ListenAddr::Tcp {
        host: "127.0.0.1".to_string(),
        port: 3000,
    },
    ..ServeConfig::default()
};

let built = build_app(VlcConfig::default(), &serve_config).await?;
let listener = bind_listener(&serve_config.main, serve_config.socket_mode).await?;

serve(listener, built, async {
    let _ = tokio::signal::ctrl_c().await;
})
.await?;
# Ok(())
# }
```

## Embedding Checklist

When embedding this crate in another binary, the binary owns process lifecycle
policy:

- Use `bind_listener` rather than binding sockets directly. It handles stale
  Unix sockets, socket permissions, and cleanup.
- Call `build_app` before `serve`. It validates the server config, warms the
  converter pool, and binds the optional admin listener.
- Pass a shutdown future to `serve`. Signal handling, parent-process EOF, and
  drain-timeout escalation are caller responsibilities.
- Install signal handlers before advertising readiness.
- Install tracing yourself or call `vl_convert_server::init_tracing`. Keep logs
  on stderr if stdout is reserved for a readiness message or other
  machine-readable output.
- Harden `VlcConfig` before calling `build_app` when serving untrusted input:
  set data-access policy, V8 heap and execution limits, plugin policy, and
  Google Fonts controls appropriate for the deployment.

## Configuration Model

The library has two configuration layers:

- `VlcConfig` controls conversion behavior: worker count, data access,
  JavaScript/V8 limits, themes/locales, plugins, local fonts, Google Fonts,
  and missing-font policy.
- `ServeConfig` controls HTTP serving behavior: listeners, bearer auth, CORS,
  request/body limits, request budgets, Google Fonts cache-miss budget
  surcharge, proxy trust, logging format, UDS permissions, admin listener
  settings, and admin reconfiguration drain time.

This crate does not parse environment variables. Embedders decide how to map
their own config files, flags, or environment variables onto `VlcConfig` and
`ServeConfig`.

`ServeConfig::default()` listens on `127.0.0.1:3000`, uses a 30 second request
timeout, caps request bodies at 50 MB, leaves auth disabled, leaves budgets
disabled, and accepts browser CORS only from loopback origins.

## Reference CLI

`vl-convert serve` is the reference binary for this crate. It adds process
behavior around the library:

- `VLC_*` environment variables and CLI flags for operational server options.
- `$PORT` fallback for the main TCP port when `VLC_PORT`/`--port` are unset.
- SIGINT/SIGTERM handling plus a drain-timeout watchdog.
- Optional `--ready-json` stdout readiness line.
- Optional parent-close shutdown for subprocess/UDS use.
- `--dump-openapi` and `--dump-openapi=admin` one-shot OpenAPI output.

For converter settings, `vl-convert serve` starts from `--vlc-config` and then
applies CLI/global `VLC_*` overrides. For server settings, it maps serve flags
and env vars directly onto `ServeConfig`; `--vlc-config` does not contain
`ServeConfig` fields.

Common `vl-convert serve` environment variables. Rows that reference
`VlcConfig` are converter settings applied before the server is built; rows
that reference `ServeConfig` are HTTP server settings.

| Env var | Library field | Default | Description |
| --- | --- | --- | --- |
| `VLC_HOST` / `VLC_PORT` | `ServeConfig.main` | `127.0.0.1:3000` | Main TCP listener. `PORT` is also honored by the CLI when `VLC_PORT` is unset. |
| `VLC_UNIX_SOCKET` | `ServeConfig.main` | unset | Main Unix socket listener. Mutually exclusive with TCP host/port. |
| `VLC_SOCKET_MODE` | `ServeConfig.socket_mode` | `0600` | Permission mode for UDS listeners. |
| `VLC_API_KEY` | `ServeConfig.api_key` | unset | Bearer token for the main API. |
| `VLC_ADMIN_PORT` / `VLC_ADMIN_HOST` | `ServeConfig.admin` | unset | Optional admin TCP listener. Defaults to loopback when only the port is set. |
| `VLC_ADMIN_UNIX_SOCKET` | `ServeConfig.admin` | unset | Optional admin Unix socket listener. |
| `VLC_ADMIN_API_KEY` | `ServeConfig.admin_api_key` | unset | Bearer token for admin routes. Required for non-loopback TCP admin. |
| `VLC_CORS_ORIGIN` | `ServeConfig.cors_origin` | loopback origins | Comma-separated allowed browser origins, `*`, or empty string to disable CORS origins. |
| `VLC_WORKERS` | `VlcConfig.num_workers` | `1` | Converter worker pool size for `vl-convert serve`. |
| `VLC_MAX_CONCURRENT_REQUESTS` | `ServeConfig.max_concurrent_requests` | unlimited | Inflight request cap; excess requests receive 503. |
| `VLC_REQUEST_TIMEOUT_SECS` | `ServeConfig.request_timeout_secs` | `30` | Per-request wall-clock timeout. |
| `VLC_RECONFIG_DRAIN_TIMEOUT_SECS` | `ServeConfig.reconfig_drain_timeout_secs` | shutdown drain value | Time to wait for active requests during admin config replacement. |
| `VLC_MAX_BODY_SIZE_MB` | `ServeConfig.max_body_size_mb` | `50` | Maximum JSON/body size. |
| `VLC_PER_IP_BUDGET_MS` | `ServeConfig.per_ip_budget_ms` | unset | Per-IP compute budget in milliseconds per minute. |
| `VLC_GLOBAL_BUDGET_MS` | `ServeConfig.global_budget_ms` | unset | Global compute budget in milliseconds per minute. |
| `VLC_BUDGET_HOLD_MS` | `ServeConfig.budget_hold_ms` | `1000` | Up-front budget reservation per request. |
| `VLC_GOOGLE_FONT_CACHE_MISS_PENALTY_MS` | `ServeConfig.google_font_cache_miss_penalty_ms` | `0` | Extra budget charged per Google Fonts CSS/font-file cache miss. |
| `VLC_TRUST_PROXY` | `ServeConfig.trust_proxy` | `false` | Trust proxy IP headers for per-IP budget accounting. |
| `VLC_OPAQUE_ERRORS` | `ServeConfig.opaque_errors` | `false` | Hide internal error details in responses. |
| `VLC_REQUIRE_USER_AGENT` | `ServeConfig.require_user_agent` | `false` | Reject API requests without `User-Agent`. |
| `VLC_LOG_FORMAT` | `ServeConfig.log_format` | `text` | `text` or `json`; logs are written to stderr. |

Reference CLI settings with no `ServeConfig` field:

| Env var | Default | Description |
| --- | --- | --- |
| `VLC_DRAIN_TIMEOUT_SECS` | `30` | Forced process-shutdown deadline after SIGINT/SIGTERM or parent-close. |
| `VLC_READY_JSON` | `false` | Emit one readiness JSON line on stdout after all listeners bind. |
| `VLC_EXIT_ON_PARENT_CLOSE` | auto for UDS | Shut down when stdin closes; auto-enabled for UDS workflows unless explicitly set. |
| `VLC_LOG_LEVEL` / `VLC_LOG_FILTER` | `warn` | Tracing filter passed to `vl-convert-server::init_tracing`; `VLC_LOG_FILTER` wins over `VLC_LOG_LEVEL`. |

Related `vl-convert serve` controls that affect conversion behavior include:
`VLC_BASE_URL`, `VLC_ALLOWED_BASE_URLS`, `VLC_FONT_DIR`,
`VLC_MAX_V8_HEAP_SIZE_MB`, `VLC_MAX_V8_EXECUTION_TIME_SECS`,
`VLC_AUTO_GOOGLE_FONTS`, `VLC_GOOGLE_FONT`,
`VLC_GOOGLE_FONT_VARIANT_THRESHOLD`, `VLC_GOOGLE_FONTS_CACHE_SIZE_MB`,
`VLC_MISSING_FONTS`, `VLC_EMBED_LOCAL_FONTS`, `VLC_SUBSET_FONTS`,
`VLC_ALLOW_GOOGLE_FONTS`, and `VLC_ALLOW_PER_REQUEST_PLUGINS`.

## Health Endpoints

| Path | Behavior |
| --- | --- |
| `/healthz` | Liveness endpoint returning `{"status":"ok"}`. |
| `/readyz` | Readiness endpoint. Returns 503 during admin reconfiguration or when a worker health probe fails. |
| `/infoz` | Version metadata, supported Vega-Lite versions, and the resolved Google Fonts cache directory. |

## Admin API

Set `ServeConfig.admin` to enable a separate admin listener. The reference CLI
does this with `--admin-port`, `--admin-host`, or `--admin-unix-socket`.

Admin routes include:

- `GET` / `POST /admin/budget` for live budget inspection and updates.
- `GET` / `PATCH` / `PUT` / `DELETE /admin/config` for live converter config.
- `GET` / `POST` / `PUT /admin/config/fonts/directories` for process font
  directory state.
- `GET` / `PUT /admin/config/fonts/cache_size` for the Google Fonts cache cap.
- `/admin/docs` and `/admin/api-doc/openapi.json` for the admin OpenAPI
  surface.

Admin auth is independent of main API auth. `ServeConfig.admin_api_key = None`
means admin access is gated only by the listener boundary: a loopback IP-literal
TCP bind such as `127.0.0.1` / `::1`, or UDS filesystem permissions.
Non-loopback TCP admin listeners require a non-empty admin API key.

## OpenAPI

The library exposes `public_openapi()` and `admin_openapi()`. The reference CLI
also provides:

```bash
vl-convert serve --dump-openapi
vl-convert serve --dump-openapi=admin
```

## Shutdown

The library `serve` function stops accepting new connections when its shutdown
future resolves and waits for spawned server tasks to exit. It does not install
signal handlers or enforce a drain deadline.

The reference `vl-convert serve` binary installs SIGINT/SIGTERM handling and
uses `VLC_DRAIN_TIMEOUT_SECS` / `--drain-timeout-secs` as the forced shutdown
deadline. For UDS/subprocess workflows it can also shut down when the parent
closes stdin.
