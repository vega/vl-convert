mod accept;
mod admin;
pub mod budget;
mod bundling;
mod config;
mod health;
pub mod json_fmt;
mod middleware;
mod router;
mod svg;
mod themes;
pub mod types;
mod util;
mod vega;
mod vegalite;

#[cfg(test)]
mod test_support;

pub(crate) use config::{apply_server_defaults, validate_serve_config, AdminConfig};
pub use config::{init_tracing, ApiKey, AppState, BuiltApp, LogFormat, ServeConfig};
pub(crate) use router::{build_middleware_stack, build_router};
pub use util::{
    append_vlc_logs_header, error_response, format_log_entries, parse_google_font_args,
    vegalite_versions,
};

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use vl_convert_rs::anyhow;
use vl_convert_rs::converter::VlcConfig;

/// Serve a [`BuiltApp`] on a pre-bound listener, spawning its background
/// tasks (budget refill, admin listener) on the current runtime and
/// draining when `shutdown` resolves. Signal handling and drain-timeout
/// escalation are the caller's responsibility — this function only
/// reacts to the injected shutdown future.
pub async fn serve(
    listener: tokio::net::TcpListener,
    built: BuiltApp,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), anyhow::Error> {
    if let Some(tracker) = built.tracker {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                tracker.refill();
            }
        });
    }
    if let Some(admin) = built.admin {
        tokio::spawn(async move {
            match tokio::net::TcpListener::bind(&admin.addr).await {
                Ok(listener) => {
                    log::info!("Admin API listening on http://{}", admin.addr);
                    let _ = axum::serve(listener, admin.router).await;
                }
                Err(e) => {
                    log::error!("Failed to bind admin port {}: {e}", admin.addr);
                }
            }
        });
    }

    axum::serve(
        listener,
        built
            .router
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await?;
    Ok(())
}

/// Build a [`BuiltApp`] from the given configuration. Runtime-free:
/// `serve` spawns the background tasks carried on the returned value.
/// Tests that exercise the router via `tower::ServiceExt::oneshot` can
/// use `built.router` directly without ever calling [`serve`].
pub fn build_app(config: VlcConfig, serve_config: &ServeConfig) -> Result<BuiltApp, anyhow::Error> {
    validate_serve_config(serve_config)?;

    let mut config = config;
    apply_server_defaults(&mut config);

    let num_workers = config.num_workers;
    log::info!("Initializing converter with {num_workers} worker(s)...");
    let converter = vl_convert_rs::converter::VlConverter::with_config(config.clone())?;
    converter.warm_up()?;
    log::info!("Workers initialized");

    let api_key = serve_config
        .api_key
        .as_ref()
        .map(|k| ApiKey::new(k.clone()));
    let state = Arc::new(AppState {
        converter: converter.clone(),
        config: config.clone(),
        api_key,
        opaque_errors: serve_config.opaque_errors,
        require_user_agent: serve_config.require_user_agent,
        readiness: health::ReadinessState::default(),
    });

    let tracker = if serve_config.per_ip_budget_ms.is_some()
        || serve_config.global_budget_ms.is_some()
        || serve_config.admin_port.is_some()
    {
        Some(budget::BudgetTracker::new(
            serve_config.per_ip_budget_ms.unwrap_or(0),
            serve_config.global_budget_ms.unwrap_or(0),
            serve_config.budget_hold_ms,
        ))
    } else {
        None
    };

    let admin = if let (Some(admin_port), Some(t)) = (serve_config.admin_port, &tracker) {
        let admin_router = admin::admin_router(t.clone())
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(TraceLayer::new_for_http())
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(CatchPanicLayer::new());
        Some(AdminConfig {
            addr: format!("127.0.0.1:{admin_port}"),
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
        converter,
        tracker,
        admin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::default_serve_config;

    #[test]
    fn test_build_app_rejects_non_positive_budget_hold_ms() {
        let config = VlcConfig::default();
        let mut serve_config = default_serve_config();
        serve_config.budget_hold_ms = 0;

        let err = build_app(config, &serve_config).err().unwrap();
        assert!(
            err.to_string().contains("budget_hold_ms must be positive"),
            "unexpected error: {err}"
        );
    }
}
