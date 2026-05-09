# vl-convert-server

Library crate for serving Vega-Lite and Vega conversions over HTTP. It wraps
`vl-convert-rs` with an axum-based REST API for SVG, PNG, PDF, JPEG, HTML,
scenegraph, theme, font, and JavaScript-bundling operations.

This crate is library-only: it does not publish a `vl-convert-server` binary.
Use the `vl-convert serve` subcommand as the reference binary, or embed the
library entry points directly.

## Quickstart

```bash
vl-convert serve --help
vl-convert serve --host 127.0.0.1 --port 3000
```

The reference CLI listens on `127.0.0.1:3000` by default. See
[Configuration](#configuration) for how to override it with flags or `VLC_*`
environment variables.

Embed as a library:

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

## Embedding checklist

When embedding the server library in another binary, keep the lifecycle
policy explicit:

- Use `bind_listener` rather than binding sockets directly. It handles stale
  Unix sockets, socket permissions, and cleanup.
- Use `build_app` before `serve`. It validates unsafe admin-listener
  combinations and warms the converter pool.
- Install signal handlers before advertising readiness.
- Keep logs on stderr if stdout is reserved for a readiness message or other
  machine-readable output.
- Pass a shutdown future to `serve` that aggregates all shutdown triggers your
  binary supports, such as SIGTERM, SIGINT, or parent-process EOF.
- Harden `VlcConfig` before calling `build_app` when serving untrusted input:
  set data-access policy, V8 heap and execution limits, and any Google Fonts
  controls required by the deployment.

## Configuration

When running through `vl-convert serve`, every serve flag has a matching
`VLC_*` environment variable. Precedence is:
**CLI flag** > `VLC_*` env var > `--vlc-config` / library config > built-in
default.

For the main TCP port only, the CLI also honors the common PaaS fallback:
`--port` > `VLC_PORT` > `PORT` > `3000`.

Key CLI options (run `vl-convert serve --help` for the full list):

| Env var                       | Default       | Description                                                                 |
|-------------------------------|---------------|-----------------------------------------------------------------------------|
| `VLC_HOST`                    | `127.0.0.1`   | Bind address. Set `0.0.0.0` inside a container.                             |
| `VLC_PORT`                    | `3000`        | Port to listen on. Falls back to `$PORT` (PaaS convention) if unset.        |
| `VLC_WORKERS`                 | config default | Number of converter worker threads.                                         |
| `VLC_MAX_V8_HEAP_SIZE_MB`     | unset         | Per-worker V8 heap cap. Unset means no cap.                                 |
| `VLC_GC_AFTER_CONVERSION`     | `false`       | Run V8 GC after each conversion (memory-reuse over throughput).             |
| `VLC_MAX_CONCURRENT_REQUESTS` | unlimited     | Inflight request cap (shed excess with 503).                                |
| `VLC_LOG_FORMAT`              | `text`        | `text` or `json`.                                                           |
| `VLC_LOG_LEVEL`               | `warn`        | Simple level: `debug`/`info`/`warn`/`error`.                                |
| `VLC_LOG_FILTER`              | (unset)       | Advanced: full `EnvFilter` directive (e.g. `vl_convert=debug,tower_http=info`). Takes precedence over `VLC_LOG_LEVEL`. |
| `VLC_TRUST_PROXY`             | `false`       | Trust `X-Envoy-External-Address` / `X-Forwarded-For` / `X-Real-IP`. Required when behind a reverse proxy for correct per-IP rate limiting. |
| `VLC_REQUEST_TIMEOUT_SECS`    | `30`          | Per-request wall clock.                                                     |
| `VLC_DRAIN_TIMEOUT_SECS`      | `30`          | Graceful-shutdown deadline after SIGTERM/SIGINT.                            |
| `VLC_PER_IP_BUDGET_MS`        | unset         | Per-IP compute-time budget in ms/min. Unset = disabled.                     |
| `VLC_GLOBAL_BUDGET_MS`        | unset         | Global compute-time budget in ms/min. Unset = disabled.                     |
| `VLC_OPAQUE_ERRORS`           | `false`       | Return status codes without error messages in production.                   |
| `VLC_ADMIN_PORT`              | unset         | Enable admin API on `127.0.0.1:<port>` for dynamic budget updates.          |

## Health endpoints

| Path       | Behavior                                                                |
|------------|-------------------------------------------------------------------------|
| `/healthz` | Liveness: static `200 OK`. Use for liveness probes.                     |
| `/readyz`  | Readiness: probes a worker with `1+1` (200 if a worker answers, 503 if the pool is unresponsive). Use for readiness / healthchecks that should gate traffic. |
| `/infoz`   | Build / version metadata.                                               |

## Graceful shutdown

On `SIGTERM` or `SIGINT` the server stops accepting new connections and
waits for in-flight requests to finish, up to `VLC_DRAIN_TIMEOUT_SECS`
(default 30 s). After the deadline, the process exits.

## Admin API

Enabling `VLC_ADMIN_PORT=N` opens an admin API on `127.0.0.1:N` for
dynamic budget inspection and updates. Bound to loopback only — not
reachable externally when the server runs in a container-hosted
environment that proxies a single port.
