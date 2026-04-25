use arc_swap::ArcSwap;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Router;
use serde_json::json;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;
use vl_convert_rs::converter::{normalize_converter_config, VlConverter, VlcConfig};

use crate::budget::{BudgetStatus, BudgetTracker};
use crate::config::{ApiKey, RuntimeSnapshot};
use crate::health::ReadinessState;
use crate::reconfig::{
    apply_patch, requires_rebuild, DrainError, PatchRejection, ReconfigCoordinator,
    ReconfigScopeGuard,
};
use crate::types::{
    ConfigPatch, ConfigReplace, ConfigValidationError, ConfigView, ErrorResponse, FieldError,
    FieldErrorCode, FontDirRequest, VlcConfigView,
};

/// OpenAPI doc for the admin surface. Published at `/admin/api-doc/openapi.json`;
/// Swagger UI at `/admin/docs`. **Separate from the main spec** — the main
/// `/api-doc/openapi.json` does not include any `/admin/*` path (guarded by
/// `test_openapi.rs::admin_paths_not_in_main_spec`). Wrappers that need to
/// codegen against the admin surface fetch this spec.
///
/// Admin endpoint body schemas are intentionally omitted from this spec for
/// now — the admin DTOs reference library types (`BaseUrlSetting`,
/// `MissingFontsPolicy`, etc.) without `ToSchema` derives, and `ConfigPatch`
/// uses an `Option<Option<T>>` tri-state pattern that doesn't reduce to a
/// clean OpenAPI schema. The spec publishes path + method + tag + response
/// status codes; wrappers consult `vl-convert-server/CLAUDE.md` §Admin
/// reconfig & drain for precise body contracts.
#[derive(OpenApi)]
#[openapi(tags(
    (name = "Admin", description = "Admin-only endpoints (config + budget)"),
))]
struct AdminApiDoc;

/// Composite state for the admin router. One `Arc<AdminState>` is cloned
/// into `.with_state(...)` for admin routes.
///
/// The `runtime` / `coordinator` / `readiness` handles are **shared by
/// Arc identity** with `AppState` — a successful commit here is
/// immediately visible to the main listener's handlers, and the gate
/// middleware on the main router participates in the same drain domain
/// as any admin-side reconfig. See `test_admin_state_composition`.
///
/// Deliberately no `font_directories` field: the registered directory
/// list lives on `snapshot.config.font_directories` after Task 0, so
/// admin handlers read it from the current runtime snapshot rather
/// than a cached parallel copy.
///
/// Design note — `/admin/budget` handlers take `State<Arc<AdminState>>`
/// rather than `State<Arc<BudgetTracker>>`. A `FromRef<Arc<AdminState>>
/// for Arc<BudgetTracker>` projection would preserve the previous
/// handler signatures, but Rust's orphan rule forbids it: neither
/// `FromRef` (axum), `Arc` (std), nor `Arc<BudgetTracker>` as a Self
/// type carries a local marker at the impl site (`Arc` isn't
/// `#[fundamental]`). Accepting the signature change keeps the state
/// plumbing single-sourced and avoids a local newtype wrapper.
//
// `runtime`, `baseline`, `coordinator`, `readiness`, `admin_api_key`,
// and `opaque_errors` are consumed by the Task 7/8/9 admin-config
// handlers — `tracker` is the only field the current-task handlers
// touch. Suppress the dead-code lint for the whole struct until those
// follower tasks land; the test_admin_state_composition unit test
// reads every field, but dead-code analysis ignores test-only reads.
#[allow(dead_code)]
pub(crate) struct AdminState {
    /// Atomic holder for the current converter + config. Shared with
    /// `AppState.runtime` by Arc identity.
    pub runtime: Arc<ArcSwap<RuntimeSnapshot>>,
    /// Snapshot-clone of the resolved startup `VlcConfig`. `DELETE
    /// /admin/config` restores this. Immutable for the process lifetime.
    pub baseline: Arc<VlcConfig>,
    /// Reconfig coordinator shared with the gate middleware. Shared
    /// with `AppState.coordinator` by Arc identity.
    pub coordinator: Arc<ReconfigCoordinator>,
    /// Readiness handle shared with `/readyz` on the main listener.
    /// Shared with `AppState.readiness` by Arc identity.
    pub readiness: Arc<ReadinessState>,
    /// Optional bearer-token credential for admin-scope auth. `None`
    /// disables admin auth — the admin listener is expected to be
    /// loopback-only or UDS-perm-guarded in that case. Plumbed through
    /// the CLI by Task 9.
    pub admin_api_key: Option<ApiKey>,
    /// Budget tracker — same `Arc` the main router's budget middleware
    /// uses, so `POST /admin/budget` mutations are observed by the
    /// main listener without a second plumbing path. Accessed by the
    /// budget handlers as `admin.tracker` (see the orphan-rule note
    /// above for why there's no `FromRef` projection).
    pub tracker: Arc<BudgetTracker>,
    /// Error-opacity toggle. When `true` admin responses omit internal
    /// detail, matching the main listener's `opaque_errors` setting.
    pub opaque_errors: bool,
}

pub(crate) fn admin_router(admin_state: Arc<AdminState>) -> Router {
    let (admin_routes, admin_api) = OpenApiRouter::with_openapi(AdminApiDoc::openapi())
        .routes(routes!(get_budget))
        .routes(routes!(update_budget))
        .routes(routes!(get_config))
        .routes(routes!(patch_config))
        .routes(routes!(put_config))
        .routes(routes!(delete_config))
        .routes(routes!(post_font_dir))
        .split_for_parts();

    admin_routes
        // Serve the admin OpenAPI spec at `/admin/api-doc/openapi.json` +
        // Swagger UI at `/admin/docs`. The path is intentionally distinct
        // from the main `/api-doc/openapi.json` so the two surfaces are
        // addressable independently.
        .merge(SwaggerUi::new("/admin/docs").url("/admin/api-doc/openapi.json", admin_api))
        // Admin auth is outermost on the admin router — every admin
        // request (including GETs + the spec endpoint) passes through it.
        // When `admin_api_key` is `None` the middleware is a no-op;
        // listener placement (UDS or TCP loopback) is the trust boundary
        // in that case.
        .layer(axum::middleware::from_fn_with_state(
            admin_state.clone(),
            crate::middleware::admin_auth_middleware,
        ))
        .with_state(admin_state)
}

#[utoipa::path(
    get,
    path = "/admin/budget",
    responses((status = 200, description = "Current budget status")),
    tag = "Admin",
)]
async fn get_budget(State(admin): State<Arc<AdminState>>) -> Json<BudgetStatus> {
    // Admin-budget handlers read `tracker` off the composite state. The
    // `Arc<BudgetTracker>` on `AdminState` is shared with the main
    // router's budget middleware by Arc identity, so mutations here are
    // observed by request admission without a second plumbing path.
    Json(admin.tracker.status())
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
struct BudgetUpdate {
    per_ip_budget_ms: Option<i64>,
    global_budget_ms: Option<i64>,
    hold_ms: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/admin/budget",
    responses(
        (status = 200, description = "Budget updated; response is fresh BudgetStatus"),
        (status = 400, description = "Invalid update body"),
    ),
    tag = "Admin",
)]
async fn update_budget(
    State(admin): State<Arc<AdminState>>,
    Json(update): Json<BudgetUpdate>,
) -> Response {
    let tracker = &admin.tracker;
    if let Some(est) = update.hold_ms {
        if est <= 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "hold_ms must be positive".to_string(),
                }),
            )
                .into_response();
        }
    }
    if let Some(per_ip) = update.per_ip_budget_ms {
        if per_ip < 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "per_ip_budget_ms must be non-negative".to_string(),
                }),
            )
                .into_response();
        }
    }
    if let Some(global) = update.global_budget_ms {
        if global < 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "global_budget_ms must be non-negative".to_string(),
                }),
            )
                .into_response();
        }
    }

    tracker.update_config(update.per_ip_budget_ms, update.global_budget_ms);
    if let Some(est) = update.hold_ms {
        tracker.update_estimate(est);
    }
    Json(tracker.status()).into_response()
}

// =============================================================================
// /admin/config handlers (Task 7)
// =============================================================================

/// Build the ConfigView response payload from a snapshot + baseline.
fn build_config_view(snapshot: &RuntimeSnapshot, baseline: &VlcConfig) -> ConfigView {
    ConfigView {
        baseline: VlcConfigView(baseline.clone()),
        effective: VlcConfigView((*snapshot.config).clone()),
        generation: snapshot.generation,
        config_version: snapshot.config_version,
    }
}

fn validation_error_from_anyhow(err: &vl_convert_rs::anyhow::Error) -> ConfigValidationError {
    let msg = err.to_string();
    ConfigValidationError {
        error: msg.clone(),
        field_errors: vec![FieldError {
            // Field path resolution is a Task 13 refinement; for now attribute
            // to the root so the response is still structurally well-formed.
            path: "".to_string(),
            code: FieldErrorCode::CrossFieldInvariant,
            message: msg,
        }],
    }
}

/// Render a ConfigValidationError as a 422 response, respecting `opaque_errors`.
fn validation_error_response(err: ConfigValidationError, opaque: bool) -> Response {
    if opaque {
        // Drop specifics but keep the 422 status so the client can still
        // distinguish validation errors from 5xx failures.
        (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({}))).into_response()
    } else {
        (StatusCode::UNPROCESSABLE_ENTITY, Json(err)).into_response()
    }
}

/// Render a non-nullable-field rejection as a 400 response. Shares the
/// ConfigValidationError payload shape with 422 responses so clients can
/// parse field-level errors uniformly.
fn non_nullable_error_response(err: ConfigValidationError, opaque: bool) -> Response {
    if opaque {
        (StatusCode::BAD_REQUEST, Json(json!({}))).into_response()
    } else {
        (StatusCode::BAD_REQUEST, Json(err)).into_response()
    }
}

fn simple_error_response(status: StatusCode, message: &str, opaque: bool) -> Response {
    if opaque {
        status.into_response()
    } else {
        (
            status,
            Json(ErrorResponse {
                error: message.to_string(),
            }),
        )
            .into_response()
    }
}

/// Convert an axum `JsonRejection` into a 400 response. serde failures for
/// unknown fields, bad types, null-on-non-nullable, or NonZero zero values
/// all funnel through here.
fn json_rejection_response(rej: JsonRejection, opaque: bool) -> Response {
    simple_error_response(StatusCode::BAD_REQUEST, &rej.body_text(), opaque)
}

/// GET /admin/config — returns the baseline, current effective config,
/// and monotonic generation + config_version counters.
#[utoipa::path(
    get,
    path = "/admin/config",
    responses((
        status = 200,
        description = "ConfigView { baseline, effective, generation, config_version }. \
                       Schema mirrors the Python get_config() shape — see vl-convert-server/CLAUDE.md §Admin reconfig & drain."
    )),
    tag = "Admin",
)]
async fn get_config(State(admin): State<Arc<AdminState>>) -> Response {
    let snap = admin.runtime.load_full();
    let view = build_config_view(&snap, &admin.baseline);
    (StatusCode::OK, Json(view)).into_response()
}

/// PATCH /admin/config — merge a partial patch onto the current config.
#[utoipa::path(
    patch,
    path = "/admin/config",
    responses(
        (status = 200, description = "Commit succeeded; response is fresh ConfigView"),
        (status = 400, description = "Malformed body / unknown field / null on non-nullable / NonZero zero"),
        (status = 422, description = "Config validation failed; response is ConfigValidationError with field_errors"),
        (status = 503, description = "Rebuild failure OR server shutting down during drain"),
        (status = 504, description = "Drain timed out; response includes in_flight count"),
    ),
    tag = "Admin",
)]
async fn patch_config(
    State(admin): State<Arc<AdminState>>,
    body: Result<Json<ConfigPatch>, JsonRejection>,
) -> Response {
    let Json(patch) = match body {
        Ok(b) => b,
        Err(rej) => return json_rejection_response(rej, admin.opaque_errors),
    };

    // Snapshot current config *before* acquiring the lock — it's fine if it
    // changes under us; we re-snapshot after the lock anyway.
    let coordinator = admin.coordinator.clone();
    let readiness = admin.readiness.clone();

    // Serialize against other admin-mutating requests. The lock guard is
    // held for the whole flow so PUT / DELETE / POST /fonts/directories
    // all see a consistent "my write committed or rolled back before the
    // next starts" ordering.
    let _lock_guard = coordinator.lock().await;
    let mut scope = ReconfigScopeGuard::new(&coordinator, &readiness);

    let current = admin.runtime.load_full();
    let new_config = match apply_patch(&current.config, &patch) {
        Ok(c) => c,
        Err(PatchRejection::NonNullable(err)) => {
            // Parse-level rejection: a non-nullable field received explicit
            // `null`. Surface as 400 alongside the other body-shape errors
            // produced by serde (unknown field, NonZero zero, etc.).
            return non_nullable_error_response(err, admin.opaque_errors);
        }
        Err(PatchRejection::Invalid(err)) => {
            return validation_error_response(err, admin.opaque_errors);
        }
    };

    run_commit(&admin, &current, new_config, &mut scope).await
}

/// PUT /admin/config — full replacement. Identical to patch_config past the
/// body-parsing stage.
#[utoipa::path(
    put,
    path = "/admin/config",
    responses(
        (status = 200, description = "Commit succeeded; response is fresh ConfigView"),
        (status = 400, description = "Malformed body / missing required field / unknown field / null on non-nullable / NonZero zero"),
        (status = 422, description = "Config validation failed; response is ConfigValidationError"),
        (status = 503, description = "Rebuild failure OR server shutting down"),
        (status = 504, description = "Drain timed out"),
    ),
    tag = "Admin",
)]
async fn put_config(
    State(admin): State<Arc<AdminState>>,
    body: Result<Json<ConfigReplace>, JsonRejection>,
) -> Response {
    let Json(replace) = match body {
        Ok(b) => b,
        Err(rej) => return json_rejection_response(rej, admin.opaque_errors),
    };
    let new_config: VlcConfig = replace.into();

    let coordinator = admin.coordinator.clone();
    let readiness = admin.readiness.clone();
    let _lock_guard = coordinator.lock().await;
    let mut scope = ReconfigScopeGuard::new(&coordinator, &readiness);
    let current = admin.runtime.load_full();
    run_commit(&admin, &current, new_config, &mut scope).await
}

/// DELETE /admin/config — reset to the startup baseline.
#[utoipa::path(
    delete,
    path = "/admin/config",
    responses(
        (status = 200, description = "Reset to baseline; response is fresh ConfigView with effective == baseline"),
        (status = 422, description = "Baseline rejected normalize_converter_config (should be impossible absent a library regression)"),
        (status = 503, description = "Rebuild failure OR server shutting down"),
        (status = 504, description = "Drain timed out"),
    ),
    tag = "Admin",
)]
async fn delete_config(State(admin): State<Arc<AdminState>>) -> Response {
    let coordinator = admin.coordinator.clone();
    let readiness = admin.readiness.clone();
    let _lock_guard = coordinator.lock().await;
    let mut scope = ReconfigScopeGuard::new(&coordinator, &readiness);
    let current = admin.runtime.load_full();
    let new_config = (*admin.baseline).clone();
    run_commit(&admin, &current, new_config, &mut scope).await
}

/// Common post-apply commit pipeline: validate, short-circuit on identity,
/// hot-apply or drain+rebuild, and return the resulting ConfigView.
async fn run_commit<'a>(
    admin: &AdminState,
    current: &Arc<RuntimeSnapshot>,
    new_config: VlcConfig,
    scope: &mut ReconfigScopeGuard<'a>,
) -> Response {
    // Normalize: validates URLs, V8 heap minimum, resolves plugin files,
    // etc. Any error here is a 422 validation failure (no globals were
    // mutated, no rollback needed).
    let new_config = match normalize_converter_config(new_config) {
        Ok(c) => c,
        Err(err) => {
            return validation_error_response(
                validation_error_from_anyhow(&err),
                admin.opaque_errors,
            );
        }
    };

    // Identity short-circuit: equal configs don't bump generation/version.
    if new_config == *current.config {
        let view = build_config_view(current, &admin.baseline);
        return (StatusCode::OK, Json(view)).into_response();
    }

    if requires_rebuild(&current.config, &new_config) {
        commit_rebuild(admin, current, new_config, scope).await
    } else {
        commit_hot_apply(admin, current, new_config)
    }
}

/// Hot-apply path: update process-global font state and swap the snapshot
/// without rebuilding the converter. `generation` unchanged;
/// `config_version` bumped.
fn commit_hot_apply(
    admin: &AdminState,
    current: &Arc<RuntimeSnapshot>,
    new_config: VlcConfig,
) -> Response {
    // Apply hot-apply fields only when they differ. Failure here is rare
    // (the library helpers return errors only for fontdb I/O failures) but
    // we surface them as 503 so the client knows the config is NOT
    // committed and can retry.
    if current.config.google_fonts_cache_size_mb != new_config.google_fonts_cache_size_mb {
        if let Err(err) = vl_convert_rs::text::apply_hot_font_cache(
            new_config.google_fonts_cache_size_mb,
        ) {
            let msg = format!(
                "failed to apply google_fonts_cache_size_mb: {err}; config not committed"
            );
            return simple_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &msg,
                admin.opaque_errors,
            );
        }
    }
    if current.config.font_directories != new_config.font_directories {
        if let Err(err) = vl_convert_rs::text::set_font_directories(&new_config.font_directories) {
            // Best effort rollback of the cache cap to keep globals in sync
            // with the snapshot we're about to refuse to swap in.
            let _ = vl_convert_rs::text::apply_hot_font_cache(
                current.config.google_fonts_cache_size_mb,
            );
            let msg = format!(
                "failed to apply font_directories: {err}; config not committed"
            );
            return simple_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                &msg,
                admin.opaque_errors,
            );
        }
    }

    let new_snapshot = Arc::new(RuntimeSnapshot {
        converter: current.converter.clone(),
        config: Arc::new(new_config),
        generation: current.generation,
        config_version: current.config_version + 1,
    });
    admin.runtime.store(new_snapshot.clone());
    let view = build_config_view(&new_snapshot, &admin.baseline);
    (StatusCode::OK, Json(view)).into_response()
}

/// Drain + rebuild path: closes the gate, waits for in-flight requests to
/// finish, snapshots prior globals for rollback, builds a new converter,
/// warms it up, and atomically swaps the snapshot. Guard-drop handles
/// gate reopening and readiness clearing on every exit path, and fires
/// the armed rollback on failure after `with_config` has mutated globals.
async fn commit_rebuild<'a>(
    admin: &AdminState,
    current: &Arc<RuntimeSnapshot>,
    new_config: VlcConfig,
    scope: &mut ReconfigScopeGuard<'a>,
) -> Response {
    let coordinator = admin.coordinator.clone();
    coordinator.close_gate();
    scope.mark_gate_closed();

    match coordinator.drain().await {
        Ok(()) => {}
        Err(DrainError::Cancelled) => {
            return simple_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "server shutting down",
                admin.opaque_errors,
            );
        }
        Err(DrainError::Timeout { inflight }) => {
            if admin.opaque_errors {
                return StatusCode::GATEWAY_TIMEOUT.into_response();
            }
            return (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({
                    "error": "drain timeout",
                    "in_flight": inflight,
                })),
            )
                .into_response();
        }
    }

    // Snapshot prior process-globals so we can restore them if
    // `with_config` succeeds (mutating globals) but a later step
    // (`warm_up`, snapshot swap) fails. `with_config` itself calls
    // `set_font_directories` + `apply_hot_font_cache` internally;
    // normalize_converter_config (called during it) runs plugin file
    // reads / URL parses but does NOT touch globals, so an error from
    // that path returns cleanly without any rollback needed.
    let prior_font_dirs = vl_convert_rs::text::current_font_directories();
    let prior_cache = vl_convert_rs::text::current_cache_size();
    scope.arm_rollback(move || {
        // Best-effort restoration; globals were mutated by with_config and
        // we want to put them back. Failures here are logged and swallowed
        // because there's no better recovery path from a Drop impl.
        if let Err(err) = vl_convert_rs::text::set_font_directories(&prior_font_dirs) {
            log::error!("rollback: set_font_directories failed: {err}");
        }
        if let Err(err) = vl_convert_rs::text::apply_hot_font_cache(prior_cache) {
            log::error!("rollback: apply_hot_font_cache failed: {err}");
        }
    });

    let new_converter = match VlConverter::with_config(new_config.clone()) {
        Ok(c) => c,
        Err(err) => {
            // with_config may have partially mutated globals before failing;
            // guard drop fires rollback.
            return validation_error_response(
                validation_error_from_anyhow(&err),
                admin.opaque_errors,
            );
        }
    };

    if let Err(err) = new_converter.warm_up() {
        let msg = format!("warm-up failed: {err}; config not committed");
        return simple_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &msg,
            admin.opaque_errors,
        );
    }

    let new_snapshot = Arc::new(RuntimeSnapshot {
        converter: new_converter,
        config: Arc::new(new_config),
        generation: current.generation + 1,
        config_version: current.config_version + 1,
    });
    admin.runtime.store(new_snapshot.clone());

    // Commit succeeded — globals already match the new config, so the
    // rollback closure must NOT run. Disarm before the guard drops.
    scope.disarm_rollback();

    let view = build_config_view(&new_snapshot, &admin.baseline);
    (StatusCode::OK, Json(view)).into_response()
}

// =============================================================================
// POST /admin/config/fonts/directories (Task 8)
// =============================================================================

/// Append a single font directory to the process-global registry and to
/// the snapshot's `VlcConfig.font_directories`. Append-only convenience for
/// callers that mirror `vlc.register_font_directory(path)` semantics —
/// replace-style modification is via `PATCH /admin/config {"font_directories": [...]}`.
///
/// Flow:
///
/// 1. Parse body (serde → 400 on malformed JSON / unknown field).
/// 2. Validate path exists and is a directory (400 otherwise).
/// 3. Acquire `coordinator.lock()` so POST serializes against PATCH / PUT /
///    DELETE — no dedup race, no lost-update vs a rebuild's global-snapshot.
/// 4. Install `ReconfigScopeGuard` for drop-safety (though this path never
///    closes the gate, a mid-call panic still needs to unwind cleanly).
/// 5. Dedup: if the path is already in `snap.config.font_directories`,
///    return 200 with the current ConfigView and no state change.
/// 6. `register_font_directory(path)` — global mutation.
/// 7. Build new `VlcConfig` with the path appended; swap a new snapshot
///    (same converter, `config_version + 1`, `generation` unchanged).
#[utoipa::path(
    post,
    path = "/admin/config/fonts/directories",
    responses(
        (status = 200, description = "Font directory appended (or already present); response is fresh ConfigView"),
        (status = 400, description = "Missing path or path not found / not a directory"),
        (status = 503, description = "Library-level register_font_directory failed; config NOT committed"),
    ),
    tag = "Admin",
)]
async fn post_font_dir(
    State(admin): State<Arc<AdminState>>,
    body: Result<Json<FontDirRequest>, JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(rej) => return json_rejection_response(rej, admin.opaque_errors),
    };

    if !req.path.is_dir() {
        return simple_error_response(
            StatusCode::BAD_REQUEST,
            &format!(
                "path not found or not a directory: {}",
                req.path.display()
            ),
            admin.opaque_errors,
        );
    }

    // Serialize against every other admin mutation. Matches the flow in
    // `run_commit` (§2.3) so a racing PATCH doesn't see a half-registered
    // font directory (global pushed, but snapshot not updated).
    let _lock = admin.coordinator.lock().await;
    let scope = ReconfigScopeGuard::new(&admin.coordinator, &admin.readiness);

    let current = admin.runtime.load_full();

    // Dedup BEFORE the register call. If the path is already tracked, the
    // library-global `FONT_CONFIG.font_dirs` also already contains it (they
    // stay in sync via `VlConverter::with_config` -> `set_font_directories`
    // at build-time and POST here at runtime), so a second
    // `register_font_directory` would be redundant.
    if current.config.font_directories.contains(&req.path) {
        drop(scope);
        let view = build_config_view(&current, &admin.baseline);
        return (StatusCode::OK, Json(view)).into_response();
    }

    let path_str = req.path.to_string_lossy();
    if let Err(err) = vl_convert_rs::text::register_font_directory(&path_str) {
        drop(scope);
        return simple_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &format!("failed to register font directory: {err}"),
            admin.opaque_errors,
        );
    }

    let mut new_config = (*current.config).clone();
    new_config.font_directories.push(req.path.clone());

    let new_snapshot = Arc::new(RuntimeSnapshot {
        converter: current.converter.clone(),
        config: Arc::new(new_config),
        generation: current.generation,
        config_version: current.config_version + 1,
    });
    admin.runtime.store(new_snapshot.clone());

    drop(scope);
    let view = build_config_view(&new_snapshot, &admin.baseline);
    (StatusCode::OK, Json(view)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppState;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    /// Asserts the Arc-identity invariants between `AppState` and
    /// `AdminState`: the three shared handles (`runtime`, `coordinator`,
    /// `readiness`) must point at the same allocations so a reconfig
    /// commit driven from the admin handlers is observed atomically by
    /// the main listener's request path — and vice versa for the gate
    /// middleware / admission accounting. Mirrors the composition
    /// `build_app` performs in `lib.rs`.
    #[test]
    fn test_admin_state_composition() {
        let runtime = Arc::new(ArcSwap::from_pointee(RuntimeSnapshot {
            converter: vl_convert_rs::converter::VlConverter::with_config(VlcConfig::default())
                .expect("construct test VlConverter"),
            config: Arc::new(VlcConfig::default()),
            generation: 0,
            config_version: 0,
        }));
        let coordinator =
            ReconfigCoordinator::new(CancellationToken::new(), Duration::from_secs(30));
        let readiness = Arc::new(ReadinessState::default());
        let tracker = BudgetTracker::new(0, 0, 1_000);
        let baseline = Arc::new(VlcConfig::default());

        let app_state = Arc::new(AppState {
            runtime: runtime.clone(),
            api_key: None,
            opaque_errors: false,
            require_user_agent: false,
            readiness: readiness.clone(),
            coordinator: coordinator.clone(),
        });

        let admin_state = Arc::new(AdminState {
            runtime: runtime.clone(),
            baseline,
            coordinator: coordinator.clone(),
            readiness: readiness.clone(),
            admin_api_key: None,
            tracker: tracker.clone(),
            opaque_errors: false,
        });

        assert!(
            Arc::ptr_eq(&app_state.runtime, &admin_state.runtime),
            "runtime handle must be shared by Arc identity between AppState and AdminState",
        );
        assert!(
            Arc::ptr_eq(&app_state.coordinator, &admin_state.coordinator),
            "coordinator handle must be shared by Arc identity between AppState and AdminState",
        );
        assert!(
            Arc::ptr_eq(&app_state.readiness, &admin_state.readiness),
            "readiness handle must be shared by Arc identity between AppState and AdminState",
        );
    }
}
