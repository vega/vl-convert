mod accept;
mod admin;
mod budget;
mod bundling;
mod config;
mod health;
mod json_fmt;
mod listen;
mod listener;
mod middleware;
// The coordinator's gate middleware + scope-guard consumers land in
// subsequent tasks (Task 4 wires the gate middleware into the main router,
// Task 6 rewrites `admin.rs` to drive drain/rebuild/commit). Until those
// tasks run, several coordinator methods and the `ScopeGuard`/`InflightGuard`
// types exist only for the (already-landed) unit tests in `reconfig.rs`.
// Suppress dead-code lints on the whole module rather than sprinkle
// `#[allow(dead_code)]` inside it — Task 2's module is explicitly off-limits
// to this task, and the warnings will clear as each follower task lands.
#[allow(dead_code)]
mod reconfig;
mod router;
mod svg;
mod themes;
mod types;
mod util;
mod vega;
mod vegalite;

#[cfg(test)]
mod test_support;

pub(crate) use config::{
    validate_serve_config, AdminConfig, ApiKey, AppState, RuntimeSnapshot,
};
pub use config::{init_tracing, BuiltApp, LogFormat, ServeConfig};
pub use listen::ListenAddr;
#[cfg(unix)]
pub use listener::UdsCleanup;
pub use listener::{bind_listener, BoundListener, EndpointInfo};
pub(crate) use router::{build_middleware_stack, build_router};

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use vl_convert_rs::anyhow;
use vl_convert_rs::converter::VlcConfig;

/// Serve a [`BuiltApp`] on a pre-bound listener, spawning its background
/// tasks (budget refill, admin listener) on the current runtime and
/// draining when `shutdown` resolves. All three (main serve, admin
/// serve, refill loop) receive the shutdown signal in parallel, and
/// `serve` only returns after every spawned task has exited — callers
/// get a deterministic "fully done" signal. Signal handling and
/// drain-timeout escalation are the caller's responsibility.
pub async fn serve(
    listener: BoundListener,
    built: BuiltApp,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), anyhow::Error> {
    // The shutdown token is constructed in `build_app` and shared with
    // the reconfig coordinator so mid-reconfig cancellations abort cleanly.
    // `serve` just fires the same token when its shutdown future resolves.
    let token = built.shutdown_token.clone();
    let mut tasks: JoinSet<()> = JoinSet::new();

    if let Some(tracker) = built.tracker {
        let cancel = token.clone();
        tasks.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = interval.tick() => tracker.refill(),
                    _ = cancel.cancelled() => break,
                }
            }
        });
    }

    if let Some(admin) = built.admin {
        let cancel = token.clone();
        tasks.spawn(async move {
            log::info!("Admin API listening on {}", admin.addr);
            let admin_shutdown = async move { cancel.cancelled().await };
            match admin.listener {
                BoundListener::Tcp(l) => {
                    let _ = axum::serve(l, admin.router)
                        .with_graceful_shutdown(admin_shutdown)
                        .await;
                }
                #[cfg(unix)]
                BoundListener::Uds(l, _cleanup) => {
                    let _ = listener::serve_uds(l, admin.router, admin_shutdown).await;
                    // `_cleanup` drops here, unlinking the admin socket.
                }
            }
        });
    }

    let shutdown_token = token;
    let shutdown = async move {
        shutdown.await;
        shutdown_token.cancel();
    };

    match listener {
        BoundListener::Tcp(l) => {
            axum::serve(
                l,
                built
                    .router
                    .into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(shutdown)
            .await?;
        }
        #[cfg(unix)]
        BoundListener::Uds(l, _cleanup) => {
            listener::serve_uds(l, built.router, shutdown).await?;
            // `_cleanup` drops here, unlinking the main socket.
        }
    }

    while tasks.join_next().await.is_some() {}
    Ok(())
}

/// Build a [`BuiltApp`] from the given configuration: warms up
/// converter workers and binds the admin listener when configured.
/// For `tower::ServiceExt::oneshot`-style tests, `built.router` can
/// be exercised directly without calling [`serve`].
pub async fn build_app(
    config: VlcConfig,
    serve_config: &ServeConfig,
) -> Result<BuiltApp, anyhow::Error> {
    use arc_swap::ArcSwap;

    validate_serve_config(serve_config)?;

    // Build the shared shutdown token first. The reconfig coordinator needs
    // it so a mid-drain cancellation unwinds through the scope guard. The
    // same token is clone-stashed on `BuiltApp` for `serve()` to drive.
    let shutdown_token = CancellationToken::new();

    let reconfig_drain_timeout = Duration::from_secs(serve_config.reconfig_drain_timeout_secs);
    let coordinator =
        reconfig::ReconfigCoordinator::new(shutdown_token.clone(), reconfig_drain_timeout);

    let num_workers = config.num_workers;
    log::info!("Initializing converter with {num_workers} worker(s)...");
    let converter = vl_convert_rs::converter::VlConverter::with_config(config)?;
    converter.warm_up()?;
    log::info!("Workers initialized");

    // Seed the admin baseline and initial `RuntimeSnapshot.config` from the
    // **normalized** view of the startup config — i.e. what the workers
    // actually run. `VlConverter::with_config` rewrites the input before
    // the workers see it (inlines file-backed `vega_plugins` source,
    // auto-populates `plugin_import_domains` from URL plugins, resolves
    // locale aliases, etc.). Seeding from the pre-normalized input would
    // make `GET /admin/config` report a stale `effective` view and let a
    // later identity PUT/DELETE unintentionally replay the unresolved
    // values. By taking `converter.config()` here we guarantee baseline ==
    // initial effective == what the workers are running, which is the
    // contract CLAUDE.md §"Admin reconfig & drain" documents for DELETE.
    let normalized = converter.config();
    let baseline = Arc::new(normalized.clone());

    let runtime = Arc::new(ArcSwap::from_pointee(RuntimeSnapshot {
        converter,
        config: Arc::new(normalized),
        generation: 0,
        config_version: 0,
    }));

    let api_key = serve_config
        .api_key
        .as_ref()
        .map(|k| ApiKey::new(k.clone()));
    let readiness = Arc::new(health::ReadinessState::default());
    let state = Arc::new(AppState {
        runtime: runtime.clone(),
        api_key,
        opaque_errors: serve_config.opaque_errors,
        require_user_agent: serve_config.require_user_agent,
        readiness: readiness.clone(),
        coordinator: coordinator.clone(),
    });

    let tracker = if serve_config.per_ip_budget_ms.is_some()
        || serve_config.global_budget_ms.is_some()
        || serve_config.admin.is_some()
    {
        Some(budget::BudgetTracker::new(
            serve_config.per_ip_budget_ms.unwrap_or(0),
            serve_config.global_budget_ms.unwrap_or(0),
            serve_config.budget_hold_ms,
        ))
    } else {
        None
    };

    let admin = if let (Some(admin_addr), Some(t)) = (&serve_config.admin, &tracker) {
        // `socket_mode` applies only to UDS listeners. Admin and main
        // share the same --socket-mode; a future --admin-socket-mode
        // flag could be added if asymmetric permissions are ever needed.
        let bound: BoundListener = bind_listener(admin_addr, serve_config.socket_mode).await?;
        let addr = bound.endpoint_label();
        // Assemble AdminState from the shared Arc'd handles. `runtime`,
        // `coordinator`, and `readiness` are intentionally the SAME Arcs
        // as on `AppState`, so admin-side reconfig commits are observed
        // by the main listener atomically. Task 9 will plumb
        // `admin_api_key` from ServeConfig; for now the field is `None`.
        let admin_state = Arc::new(admin::AdminState {
            runtime: runtime.clone(),
            baseline: baseline.clone(),
            coordinator: coordinator.clone(),
            readiness: readiness.clone(),
            admin_api_key: serve_config
                .admin_api_key
                .as_ref()
                .map(|k| ApiKey::new(k.clone())),
            tracker: t.clone(),
            opaque_errors: serve_config.opaque_errors,
        });
        let admin_router = admin::admin_router(admin_state)
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(TraceLayer::new_for_http())
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(CatchPanicLayer::new());
        Some(AdminConfig {
            listener: bound,
            addr,
            router: admin_router,
        })
    } else {
        None
    };

    let router = build_router(
        state,
        tracker.clone(),
        serve_config.opaque_errors,
        serve_config.trust_proxy,
    );
    let app = build_middleware_stack(router, serve_config);

    Ok(BuiltApp {
        router: app,
        runtime,
        shutdown_token,
        tracker,
        admin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::default_serve_config;

    #[tokio::test]
    async fn test_build_app_rejects_non_positive_budget_hold_ms() {
        let config = VlcConfig::default();
        let mut serve_config = default_serve_config();
        serve_config.budget_hold_ms = 0;

        let err = build_app(config, &serve_config).await.err().unwrap();
        assert!(
            err.to_string().contains("budget_hold_ms must be positive"),
            "unexpected error: {err}"
        );
    }
}
