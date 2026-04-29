# vl-convert-server

HTTP server for converting Vega-Lite and Vega specifications to static images
(SVG, PNG, PDF, JPEG). Wraps the `vl-convert-rs` library with an axum-based
REST API.

## Quickstart

```bash
vl-convert-server --help
```

The server listens on `127.0.0.1:3000` by default. See
[Configuration](#configuration) for how to override via env vars or CLI
flags.

## Configuration

Every CLI flag has a matching `VLC_*` environment variable. Precedence:
**CLI flag** > `VLC_*` env var > (for port only: `PORT` env var fallback) >
built-in default.

Key options (run `--help` for the full list):

| Env var                       | Default       | Description                                                                 |
|-------------------------------|---------------|-----------------------------------------------------------------------------|
| `VLC_HOST`                    | `127.0.0.1`   | Bind address. Set `0.0.0.0` inside a container.                             |
| `VLC_PORT`                    | `3000`        | Port to listen on. Falls back to `$PORT` (PaaS convention) if unset.        |
| `VLC_WORKERS`                 | `min(CPU, 4)` | Number of converter worker threads.                                         |
| `VLC_MAX_V8_HEAP_SIZE_MB`     | `512`         | Per-worker V8 heap cap.                                                     |
| `VLC_GC_AFTER_CONVERSION`     | `false`       | Run V8 GC after each conversion (memory-reuse over throughput).             |
| `VLC_MAX_CONCURRENT_REQUESTS` | unlimited     | Inflight request cap (shed excess with 503).                                |
| `VLC_LOG_FORMAT`              | `text`        | `text` or `json`.                                                           |
| `VLC_LOG_LEVEL`               | `info`        | Simple level: `trace`/`debug`/`info`/`warn`/`error`.                        |
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
