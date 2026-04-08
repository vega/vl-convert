mod common;

use common::*;
use once_cell::sync::Lazy;
use serde_json;

static PER_IP_BUDGET_SERVER: Lazy<(TestServer, u16)> =
    Lazy::new(|| start_budget_server(Some(1), None, 2000));

#[tokio::test]
async fn test_budget_rate_limit_triggered() {
    let (server, _admin_port) = &*PER_IP_BUDGET_SERVER;
    // With per_ip_budget_ms=1 and budget_hold_ms=2000, the very first request
    // should exhaust the budget. Send a request to trigger reservation.
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    // The first request may succeed or fail depending on timing; the second
    // should definitely be rate-limited.
    let resp2 = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    // At least one of them should be 429
    assert!(
        resp.status() == 429 || resp2.status() == 429,
        "expected at least one 429 response, got: {} and {}",
        resp.status(),
        resp2.status()
    );
}

#[tokio::test]
async fn test_budget_health_not_rate_limited() {
    let (server, _admin_port) = &*PER_IP_BUDGET_SERVER;
    // Health endpoints should bypass budget tracking
    for _ in 0..5 {
        let resp = server
            .client
            .get(format!("{}/healthz", server.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "health should never be rate limited");
    }
}

#[tokio::test]
async fn test_budget_admin_get() {
    let (_server, admin_port) = &*PER_IP_BUDGET_SERVER;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["per_ip_budget_ms"], 1);
    assert_eq!(body["hold_ms"], 2000);
}

#[tokio::test]
async fn test_budget_admin_update() {
    let (_server, admin_port) = &*PER_IP_BUDGET_SERVER;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .json(&serde_json::json!({"hold_ms": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["hold_ms"], 500);
}

static GLOBAL_BUDGET_SERVER: Lazy<(TestServer, u16)> =
    Lazy::new(|| start_budget_server(None, Some(1), 2000));

#[tokio::test]
async fn test_budget_global_depletion() {
    let (server, _admin_port) = &*GLOBAL_BUDGET_SERVER;
    // With global_budget_ms=1 and budget_hold_ms=2000, the first request
    // should immediately deplete the global budget.
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        429,
        "expected 429 when global budget is exhausted, got: {}",
        resp.status()
    );
}

static NO_BUDGET_ADMIN_SERVER: Lazy<(TestServer, u16)> =
    Lazy::new(|| start_budget_server(None, None, 2000));

#[tokio::test]
async fn test_budget_admin_enable() {
    let (server, admin_port) = &*NO_BUDGET_ADMIN_SERVER;
    // Without any budget configured, requests should succeed
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 when no budget is set, got: {}",
        resp.status()
    );

    // Enable a very tight per-IP budget via admin API
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .json(&serde_json::json!({"per_ip_budget_ms": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Now requests should be rate-limited
    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    let resp2 = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status() == 429 || resp2.status() == 429,
        "expected at least one 429 after enabling budget, got: {} and {}",
        resp.status(),
        resp2.status()
    );
}

static ESTIMATE_UPDATE_SERVER: Lazy<(TestServer, u16)> =
    Lazy::new(|| start_budget_server(Some(1), None, 2000));

#[tokio::test]
async fn test_budget_admin_estimate_update() {
    let (_server, admin_port) = &*ESTIMATE_UPDATE_SERVER;
    let client = reqwest::Client::new();

    // Update hold_ms via POST
    let resp = client
        .post(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .json(&serde_json::json!({"hold_ms": 750}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify via GET
    let resp = client
        .get(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["hold_ms"], 750,
        "expected hold_ms to be updated to 750, got: {}",
        body["hold_ms"]
    );
}
