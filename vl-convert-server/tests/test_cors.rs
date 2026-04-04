mod common;

use common::*;

#[tokio::test]
async fn test_cors_localhost_origin_allowed() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/themes", server.base_url),
        )
        .header("origin", "http://localhost:3000")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        acao, "http://localhost:3000",
        "expected localhost origin to be reflected"
    );
}

#[tokio::test]
async fn test_cors_127_origin_allowed() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/themes", server.base_url),
        )
        .header("origin", "http://127.0.0.1:8080")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        acao, "http://127.0.0.1:8080",
        "expected 127.0.0.1 origin to be reflected"
    );
}

#[tokio::test]
async fn test_cors_remote_origin_rejected() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/themes", server.base_url),
        )
        .header("origin", "https://evil.example.com")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    let acao = resp.headers().get("access-control-allow-origin");
    assert!(
        acao.is_none(),
        "expected remote origin to be rejected (no ACAO header), got: {:?}",
        acao.map(|v| v.to_str().ok())
    );
}
