use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

use crate::budget::{BudgetStatus, BudgetTracker};
use crate::types::ErrorResponse;

pub fn admin_router(tracker: Arc<BudgetTracker>) -> Router {
    Router::new()
        .route("/admin/budget", get(get_budget))
        .route("/admin/budget", post(update_budget))
        .with_state(tracker)
}

async fn get_budget(State(tracker): State<Arc<BudgetTracker>>) -> Json<BudgetStatus> {
    Json(tracker.status())
}

#[derive(serde::Deserialize)]
struct BudgetUpdate {
    per_ip_budget_ms: Option<i64>,
    global_budget_ms: Option<i64>,
    estimate_ms: Option<i64>,
}

async fn update_budget(
    State(tracker): State<Arc<BudgetTracker>>,
    Json(update): Json<BudgetUpdate>,
) -> Response {
    if let Some(est) = update.estimate_ms {
        if est <= 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "estimate_ms must be positive".to_string(),
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
    if let Some(est) = update.estimate_ms {
        tracker.update_estimate(est);
    }
    Json(tracker.status()).into_response()
}
