# vl-convert-server

Library crate that builds an axum HTTP API around `VlConverter`. Public
routes, operator-facing behavior, and embedding basics belong in
`README.md` and OpenAPI; keep this file focused on implementation
invariants that are easy to regress.

## Boundaries

- This crate is library-only. It has no `src/main.rs` and no `[[bin]]`
  target.
- Public entry points are `build_app`, `bind_listener`, `serve`, and
  OpenAPI helpers from `src/lib.rs`.
- The reference binary is `../vl-convert/src/serve.rs`, which owns CLI
  parsing, env fallback, ready JSON, signal handling, drain watchdogs,
  OpenAPI dumping, and parent-close shutdown.
- Do not duplicate route inventories here. Route registration is in
  `src/router.rs`; generated docs come from `#[utoipa::path]` handler
  annotations and DTO `ToSchema` derives.

## App Construction

`build_app(VlcConfig, &ServeConfig)` is the construction boundary. It
validates `ServeConfig`, warms a `VlConverter`, creates shared state,
binds the optional admin listener through `bind_listener`, and composes
the router/middleware stack.

Do not bypass `build_app` in downstream binaries. It runs the non-loopback
admin listener guard and warmup path that callers should inherit.

`build_app` does not harden the supplied `VlcConfig`. Defaults such as an
open HTTP/HTTPS data allowlist, no V8 heap cap, and no V8 execution timer
match the CLI surface. Callers that need SSRF protection or finite V8
resources must set `allowed_base_urls`, `max_v8_heap_size_mb`, and
`max_v8_execution_time_secs` before calling `build_app`.

`tower::TimeoutLayer` only drops the response future. It does not stop V8
execution; use `max_v8_execution_time_secs` when runaway specs must be
terminated.

`BuiltApp.router` is a standalone `tower::Service`, so tests may call
`router.oneshot(req)` without `serve()`. `BuiltApp.admin` travels with the
main app into `serve()`, which spawns the admin listener beside the main
listener.

## Handler Rules

Conversion handlers should follow this shape:

1. Load one runtime snapshot with `state.runtime.load_full()` at handler
   entry.
2. Validate common request options against that same snapshot config.
3. Call the matching `VlConverter` method.
4. On success, attach converter logs and `output.font_stats` where the
   output type carries them.
5. On conversion failure, use `conversion_error_response` so partial
   Google Fonts stats from the error chain still reach response
   extensions.

`error_response` is for request validation and other non-converter errors.
Budget charging is middleware-owned; handlers should only attach the stats
and logs needed by that middleware.

Health routes (`/healthz`, `/readyz`, `/infoz`) intentionally bypass auth,
user-agent, budget, and reconfig-gate middleware.

## Tests

Integration tests share `tests/common/mod.rs`.

- `start_server_sync` binds `127.0.0.1:0` synchronously on the test
  thread before handing the listener to a per-test `current_thread`
  runtime. Keep that handoff to avoid TOCTOU port races.
- UDS tests must bind through `bind_listener` and keep the socket tempdir
  alive for the server lifetime.
- reqwest has no UDS transport at the workspace pin; use the existing raw
  hyper + `tokio::net::UnixStream` helpers.
- Run tests single-threaded (`--test-threads=1`) because Deno/V8 test
  state is not safe to exercise concurrently.

This crate has no subprocess e2e tests because it ships no binary. Put
binary lifecycle tests in the downstream binary crate.

## Listener and UDS Lifecycle

Keep `ListenAddr` (`src/listen.rs`) distinct from `BoundListener`
(`src/listener.rs`). `ListenAddr` is config/CLI-shaped intent.
`BoundListener` owns runtime lifecycle, including `UdsCleanup`.

Every pathname UDS bind in this crate must go through
`bind_listener(&ListenAddr, mode)`, never raw `UnixListener::bind`.
That helper centralizes stale-socket probing, bind, immediate chmod, and
unlink-on-drop cleanup. There must be no `await` between bind and chmod.

Force-exit through the drain watchdog bypasses Drop; the next launch's
probe-then-unlink path clears stale socket files.

The library exposes exactly one serve entrypoint:

```rust
pub async fn serve(listener: BoundListener, built: BuiltApp, shutdown: impl Future<Output = ()> + Send + 'static)
```

Do not add a parallel `serve_uds`. The cancellation/drain composition
must remain single-sourced.

## UDS Auth and Observability

UDS filesystem permissions are the trust boundary. Peer credentials are
observability only.

`Connected<&UnixStream> for UdsConnectInfo` must keep
`stream.peer_cred().ok()`. Missing credentials leave tracing fields empty
and allow the request to continue. Do not turn this into `expect`; some
sandboxed kernels or future tokio credential probes may fail even when the
request itself is valid.

## Tracing and Budget Stats

`tracing_subscriber::fmt()` writes to stdout by default; `init_tracing`
must force stderr. Downstream binaries often reserve stdout for one
machine-readable readiness line or conversion output.

Google Fonts stats are accounting data. Handlers attach them to response
extensions on success, and `conversion_error_response` extracts them from
conversion errors on failure. The budget middleware reads those extensions
to record `google_font.*` tracing fields and apply
`google_font_cache_miss_penalty_ms`.

When rate-limiting state is keyed by peer IP, preserve `Option<IpAddr>`
through reservation, adjustment, and refund paths. Do not replace missing
IP data with `0.0.0.0`; UDS and tests are legitimate no-IP cases. `None`
skips only the per-IP dimension; the global budget still applies.

## Admin Reconfiguration

Runtime config lives behind the admin listener. The active state is an
`ArcSwap<RuntimeSnapshot>` shared by main and admin state; handlers load
one snapshot at entry and use it through the request.

The admin baseline is captured from `converter.config()` after
`VlConverter::with_config` normalization. Do not seed baseline from raw
CLI input; normalized baseline must equal the initial effective config and
what workers actually run.

Every successful non-identity `/admin/config` commit uses
drain-then-rebuild-then-swap:

1. Close the reconfig gate.
2. Drain admitted in-flight requests within `reconfig_drain_timeout_secs`.
3. Reconfigure/build and warm the new converter.
4. Store a new snapshot and bump `generation`.
5. Reopen the gate.

The admission handshake is increment-first, recheck-after. A request must
increment `inflight` before checking whether the gate closed, otherwise it
can be admitted after the drain loop observes zero in-flight work.

The drain loop registers `Notify::notified()` before reading the counter
and uses one absolute deadline for the whole drain window.

`reconfig_gate_middleware` is the last layer added to the API router so it
runs outermost. Gate-closed 503 responses skip budget, auth, and UA
middleware. Health routes remain outside the API stack.

`ReconfigScopeGuard` is required once a handler can close the gate. It
reopens the gate and clears readiness on early return, `?`, panic caught by
`CatchPanicLayer`, or client-disconnect drop.

SIGTERM/SIGINT should win over in-progress reconfig. `drain()` selects
against the same shutdown token that `serve()` watches, with shutdown
biased ahead of notify/deadline events.

## Config Surface Rules

Admin auth is separate from main API auth. `admin_api_key` is optional for
UDS and loopback TCP, but non-loopback TCP admin without a non-blank key
must remain a `validate_serve_config` error.

JSON `null` maps naturally to nullable config fields such as
`max_v8_heap_size_mb`. Non-nullable fields such as `allowed_base_urls`
must reject null with 400. Field-level PATCH has no "reset to library
default" primitive; use `DELETE /admin/config` or pass the explicit
default.

Process-global state stays out of `VlcConfig`, `ConfigPatch`,
`ConfigReplace`, and `ConfigView`. Current examples are font directories
and the Google Fonts cache size, each with dedicated
`/admin/config/fonts/...` endpoints. They serialize against the reconfig
coordinator lock but do not close the gate or rebuild the converter.

Read-only process/system state belongs on `/infoz`, not `/admin/config`.
`google_fonts_cache_dir` is the model: it is resolved at process start,
reported by `/infoz`, and guarded by `test_infoz_surface_unchanged`.

## Downstream Binaries

Public embedding guidance belongs in `README.md`. Agent-facing rule of
thumb: downstream binaries may customize lifecycle policy, but they should
still compose the library through `bind_listener`, `build_app`, and
`serve`, and should preserve stderr tracing when stdout carries structured
readiness or conversion output.
