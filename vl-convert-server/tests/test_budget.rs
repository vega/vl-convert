mod common;

use common::*;
use serde_json;

#[tokio::test]
async fn test_budget_rate_limit_triggered() {
    let (server, _admin_port) = &*BUDGET_SERVER;
    // With per_ip_budget_ms=1 and budget_estimate_ms=2000, the very first request
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
    let (server, _admin_port) = &*BUDGET_SERVER;
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
    let (_server, admin_port) = &*BUDGET_SERVER;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["per_ip_budget_ms"], 1);
    assert_eq!(body["estimate_ms"], 2000);
}

#[tokio::test]
async fn test_budget_admin_update() {
    let (_server, admin_port) = &*BUDGET_SERVER;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{admin_port}/admin/budget"))
        .json(&serde_json::json!({"estimate_ms": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["estimate_ms"], 500);
}
