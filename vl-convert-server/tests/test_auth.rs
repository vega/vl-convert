mod common;

use common::*;

#[tokio::test]
async fn test_auth_healthz_no_key_needed() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "health endpoints should be accessible without auth"
    );
}

#[tokio::test]
async fn test_auth_api_rejected_without_valid_key() {
    let server = &*AUTH_SERVER;
    for auth_header in [None, Some("Bearer wrong-key")] {
        let mut req = server.client.get(format!("{}/themes", server.base_url));
        if let Some(auth_header) = auth_header {
            req = req.header("authorization", auth_header);
        }
        let resp = req.send().await.unwrap();
        assert_eq!(resp.status(), 401, "auth_header={auth_header:?}");
        assert!(
            resp.headers().get("www-authenticate").is_some(),
            "expected WWW-Authenticate header on 401; auth_header={auth_header:?}"
        );
    }
}

#[tokio::test]
async fn test_auth_api_accepted_with_correct_key() {
    let server = &*AUTH_SERVER;
    let resp = server
        .client
        .get(format!("{}/themes", server.base_url))
        .header("authorization", "Bearer test-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "expected 200 with correct API key, got: {}",
        resp.status()
    );
}
