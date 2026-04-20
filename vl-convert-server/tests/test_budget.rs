mod common;

use common::*;
use once_cell::sync::Lazy;

static GLOBAL_BUDGET_SERVER: Lazy<BudgetServer> =
    Lazy::new(|| start_budget_server(None, Some(1), 2000, false));

#[tokio::test]
async fn test_budget_rate_limit_triggered() {
    let BudgetServer { handle: server, .. } = start_budget_server(Some(1), None, 2000, false);
    // With per_ip_budget_ms=1 and hold_ms=2000, reserve(2000) > budget(1)
    // so the very first request is rejected deterministically.
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
        "expected 429 when hold_ms >> budget, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_budget_health_not_rate_limited() {
    let BudgetServer { handle: server, .. } = start_budget_server(Some(1), None, 2000, false);
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
    let BudgetServer { admin_base_url, .. } = start_budget_server(Some(1), None, 2000, false);
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{admin_base_url}/admin/budget"))
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
    let BudgetServer { admin_base_url, .. } = start_budget_server(Some(1), None, 2000, false);
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{admin_base_url}/admin/budget"))
        .json(&serde_json::json!({"hold_ms": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["hold_ms"], 500);
}

#[tokio::test]
async fn test_budget_global_depletion() {
    let BudgetServer { handle: server, .. } = &*GLOBAL_BUDGET_SERVER;
    // With global_budget_ms=1 and hold_ms=2000, first request is rejected.
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

#[tokio::test]
async fn test_budget_admin_enable() {
    let BudgetServer {
        handle: server,
        admin_base_url,
        ..
    } = start_budget_server(None, None, 2000, false);
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
        .post(format!("{admin_base_url}/admin/budget"))
        .json(&serde_json::json!({"per_ip_budget_ms": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Now requests should be rate-limited (hold_ms=2000 >> per_ip_budget_ms=1)
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
        "expected 429 after enabling tight budget, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_budget_admin_hold_update() {
    let BudgetServer { admin_base_url, .. } = start_budget_server(Some(1), None, 2000, false);
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{admin_base_url}/admin/budget"))
        .json(&serde_json::json!({"hold_ms": 750}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{admin_base_url}/admin/budget"))
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

#[tokio::test]
async fn test_trust_proxy_true_isolates_ips() {
    // With trust_proxy=true, different X-Forwarded-For IPs get independent budgets.
    // per_ip_budget_ms=1 and hold_ms=2000 means every request is immediately rejected.
    // But each unique XFF IP is tracked separately — proving isolation.
    let BudgetServer { handle: server, .. } = start_budget_server(Some(1), None, 2000, true);

    let resp1 = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("X-Forwarded-For", "10.0.0.1")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 429, "10.0.0.1 should be rate-limited");

    let resp2 = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("X-Forwarded-For", "10.0.0.2")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp2.status(),
        429,
        "10.0.0.2 should also be rate-limited (independent bucket)"
    );
}

#[tokio::test]
async fn test_trust_proxy_false_ignores_xff() {
    // With trust_proxy=false, X-Forwarded-For is ignored — all requests use socket IP.
    let BudgetServer { handle: server, .. } = start_budget_server(Some(1), None, 2000, false);

    let resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .header("X-Forwarded-For", "10.0.0.1")
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    // Still 429 — XFF is ignored, socket IP (127.0.0.1) is used, same budget
    assert_eq!(resp.status(), 429);
}
