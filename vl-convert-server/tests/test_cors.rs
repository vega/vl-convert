mod common;

use common::*;

#[tokio::test]
async fn test_cors_default_loopback_policy() {
    let server = &*DEFAULT_SERVER;
    let cases = [
        ("http://localhost:3000", Some("http://localhost:3000")),
        ("http://127.0.0.1:8080", Some("http://127.0.0.1:8080")),
        ("https://evil.example.com", None),
    ];

    for (origin, expected_acao) in cases {
        let resp = server
            .client
            .request(
                reqwest::Method::OPTIONS,
                format!("{}/themes", server.base_url),
            )
            .header("origin", origin)
            .header("access-control-request-method", "GET")
            .send()
            .await
            .unwrap();
        let acao = resp
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok());
        assert_eq!(acao, expected_acao, "origin={origin}");
    }
}
