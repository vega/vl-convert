use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use std::sync::Arc;

use crate::util::error_response;
use crate::AppState;

pub(crate) async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    if let Some(ref key) = state.api_key {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        let authorized = match auth_header {
            Some(val)
                if val
                    .get(..7)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer ")) =>
            {
                key.matches(&val.as_bytes()[7..])
            }
            _ => false,
        };

        if !authorized {
            let mut resp = error_response(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                state.opaque_errors,
            );
            resp.headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
            return resp;
        }
    }
    next.run(req).await
}

pub(crate) async fn user_agent_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Response {
    if state.require_user_agent {
        let ua = req
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if ua.is_empty() {
            return error_response(
                StatusCode::BAD_REQUEST,
                "User-Agent header is required",
                state.opaque_errors,
            );
        }
    }
    next.run(req).await
}

/// Returns true if `ip` is a loopback, private-range, link-local,
/// unspecified, or CGNAT address. Used to skip internal hops when
/// walking `X-Forwarded-For` right-to-left.
pub(crate) fn is_private_or_loopback(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let [a, b, _, _] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                // CGNAT 100.64.0.0/10 (RFC 6598) — used by Railway's
                // internal network, AWS NAT, mobile carriers, etc.
                || (a == 100 && (64..=127).contains(&b))
        }
        std::net::IpAddr::V6(v6) => {
            let first = v6.octets()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique local fc00::/7
                || (first & 0xfe) == 0xfc
                // Link-local fe80::/10
                || (first == 0xfe && (v6.octets()[1] & 0xc0) == 0x80)
        }
    }
}

/// Extract client IP.
///
/// When `trust_proxy` is true, prefers (in order):
/// 1. `X-Envoy-External-Address` — single trusted client IP on
///    Envoy-based proxies (Railway's edge, Google Cloud Run, etc.).
/// 2. `X-Forwarded-For` — walked **right-to-left** (appending proxies
///    place the client hop toward the right); skips private/loopback
///    entries until a public address is found. If every parseable
///    entry is private, returns the rightmost parseable one.
/// 3. `X-Real-IP` — nginx convention.
/// 4. Peer socket address.
///
/// When `trust_proxy` is false, always uses the peer socket address.
///
/// Taking the leftmost XFF entry is **unsafe** on any appending proxy
/// (Railway, nginx, envoy, ALB): an attacker can spoof the client hop
/// by sending their own `X-Forwarded-For`. This implementation walks
/// right-to-left to land on the first trusted hop.
///
/// Returning `None` is legitimate in two cases: (1) the request came in
/// over a UDS listener (no peer IP exists at the socket layer); (2) the
/// request was built directly via `Request::builder()` in a test
/// harness that didn't inject `ConnectInfo<SocketAddr>`. Callers
/// MUST NOT fall back to `Ipv4Addr::UNSPECIFIED` / `0.0.0.0` on
/// `None` — doing so would collapse every non-IP caller into a single
/// shared per-IP bucket. The budget middleware threads
/// `Option<IpAddr>` through `reserve` / `apply_adjustment`; `None`
/// correctly skips the per-IP dimension while the global dimension
/// still applies.
pub(crate) fn extract_client_ip(
    req: &axum::http::Request<axum::body::Body>,
    trust_proxy: bool,
) -> Option<std::net::IpAddr> {
    if trust_proxy {
        // X-Envoy-External-Address: a single trusted client IP.
        if let Some(hdr) = req.headers().get("x-envoy-external-address") {
            if let Ok(s) = hdr.to_str() {
                if let Ok(ip) = s.trim().parse::<std::net::IpAddr>() {
                    return Some(ip);
                }
            }
        }
        // X-Forwarded-For: walk right-to-left, prefer first public entry.
        if let Some(xff) = req.headers().get("x-forwarded-for") {
            if let Ok(xff_str) = xff.to_str() {
                let parsed: Vec<std::net::IpAddr> = xff_str
                    .split(',')
                    .filter_map(|part| part.trim().parse::<std::net::IpAddr>().ok())
                    .collect();
                if let Some(public) = parsed.iter().rev().find(|ip| !is_private_or_loopback(ip)) {
                    return Some(*public);
                }
                if let Some(last) = parsed.last() {
                    return Some(*last);
                }
                // Header was present but had no parseable entries — fall
                // through to X-Real-IP / peer rather than returning None.
            }
        }
        // X-Real-IP: nginx convention; fallback after XFF / Envoy yield
        // nothing.
        if let Some(xri) = req.headers().get("x-real-ip") {
            if let Ok(ip_str) = xri.to_str() {
                if let Ok(ip) = ip_str.trim().parse::<std::net::IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }
    // Peer socket address (always available, always trustworthy).
    req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(headers: &[(&str, &str)]) -> axum::http::Request<axum::body::Body> {
        let mut builder = axum::http::Request::builder().method("GET").uri("/test");
        for &(key, val) in headers {
            builder = builder.header(key, val);
        }
        builder.body(axum::body::Body::empty()).unwrap()
    }

    #[test]
    fn test_extract_ip_trust_proxy_false_ignores_xff() {
        let req = make_request(&[("x-forwarded-for", "10.0.0.1")]);
        let ip = extract_client_ip(&req, false);
        assert_eq!(
            ip, None,
            "trust_proxy=false should ignore XFF and return None (no ConnectInfo)"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_false_ignores_x_real_ip() {
        let req = make_request(&[("x-real-ip", "10.0.0.1")]);
        let ip = extract_client_ip(&req, false);
        assert_eq!(ip, None);
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_xff_single_entry() {
        let req = make_request(&[("x-forwarded-for", "10.0.0.1")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(ip, Some("10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_xff_all_private_returns_rightmost() {
        let req = make_request(&[("x-forwarded-for", "10.0.0.1, 10.0.0.99, 10.0.0.100")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("10.0.0.100".parse().unwrap()),
            "all-private chain should fall back to rightmost parseable"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_xff_attacker_prepended() {
        // Security regression: an attacker sends X-Forwarded-For: 9.9.9.9
        // and Railway's edge appends its hop — the leftmost entry is
        // attacker-controlled, the rightmost public entry is the truth.
        let req = make_request(&[("x-forwarded-for", "9.9.9.9, 203.0.113.7")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("203.0.113.7".parse().unwrap()),
            "rightmost public entry must win over attacker-prepended leftmost"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_xff_mixed_private_public() {
        // Skip CGNAT (100.64/10 — Railway's internal range), RFC1918,
        // and return the rightmost non-private hop.
        let req = make_request(&[("x-forwarded-for", "8.8.8.8, 10.0.0.1, 100.64.5.7")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("8.8.8.8".parse().unwrap()),
            "should skip CGNAT and RFC1918 walking right-to-left"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_x_real_ip() {
        let req = make_request(&[("x-real-ip", "192.168.1.1")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(ip, Some("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_xff_preferred_over_x_real_ip() {
        let req = make_request(&[
            ("x-forwarded-for", "10.0.0.1"),
            ("x-real-ip", "192.168.1.1"),
        ]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("10.0.0.1".parse().unwrap()),
            "XFF should take precedence"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_invalid_xff_falls_back_to_x_real_ip() {
        let req = make_request(&[
            ("x-forwarded-for", "not-an-ip"),
            ("x-real-ip", "192.168.1.1"),
        ]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("192.168.1.1".parse().unwrap()),
            "invalid XFF should fall back to X-Real-IP"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_empty_xff() {
        let req = make_request(&[("x-forwarded-for", "")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip, None,
            "empty XFF with no X-Real-IP and no ConnectInfo should return None"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_ipv6() {
        let req = make_request(&[("x-forwarded-for", "2001:db8::1")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(ip, Some("2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_no_headers() {
        let req = make_request(&[]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip, None,
            "no proxy headers and no ConnectInfo should return None"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_false_ignores_envoy_external() {
        let req = make_request(&[("x-envoy-external-address", "203.0.113.1")]);
        let ip = extract_client_ip(&req, false);
        assert_eq!(ip, None);
    }

    #[test]
    fn test_extract_ip_envoy_external_address_wins_over_xff() {
        let req = make_request(&[
            ("x-envoy-external-address", "203.0.113.1"),
            ("x-forwarded-for", "1.1.1.1"),
        ]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("203.0.113.1".parse().unwrap()),
            "Envoy header should take precedence over XFF"
        );
    }

    #[test]
    fn test_extract_ip_envoy_external_address_invalid_falls_back_to_xff() {
        let req = make_request(&[
            ("x-envoy-external-address", "not-an-ip"),
            ("x-forwarded-for", "1.1.1.1"),
        ]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("1.1.1.1".parse().unwrap()),
            "invalid Envoy header should fall through to XFF"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_ipv6_loopback_skipped() {
        let req = make_request(&[("x-forwarded-for", "2001:db8::1, ::1")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("2001:db8::1".parse().unwrap()),
            "IPv6 loopback should be skipped walking right-to-left"
        );
    }

    #[test]
    fn test_extract_ip_trust_proxy_true_ipv6_ula_skipped() {
        let req = make_request(&[("x-forwarded-for", "2606:4700::1, fc00::10")]);
        let ip = extract_client_ip(&req, true);
        assert_eq!(
            ip,
            Some("2606:4700::1".parse().unwrap()),
            "IPv6 unique-local (fc00::/7) should be skipped"
        );
    }

    #[test]
    fn test_is_private_or_loopback_ipv4() {
        let private: &[&str] = &[
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "169.254.0.1",
            "0.0.0.0",
        ];
        for s in private {
            let ip: std::net::IpAddr = s.parse().unwrap();
            assert!(is_private_or_loopback(&ip), "{s} should be private");
        }
        let public: &[&str] = &["8.8.8.8", "203.0.113.7", "100.63.255.255", "100.128.0.0"];
        for s in public {
            let ip: std::net::IpAddr = s.parse().unwrap();
            assert!(!is_private_or_loopback(&ip), "{s} should be public");
        }
    }

    #[test]
    fn test_is_private_or_loopback_ipv6() {
        let private: &[&str] = &["::1", "fc00::1", "fd00::1", "fe80::1", "::"];
        for s in private {
            let ip: std::net::IpAddr = s.parse().unwrap();
            assert!(is_private_or_loopback(&ip), "{s} should be private");
        }
        let public: &[&str] = &["2001:db8::1", "2606:4700::1"];
        for s in public {
            let ip: std::net::IpAddr = s.parse().unwrap();
            assert!(!is_private_or_loopback(&ip), "{s} should be public");
        }
    }
}
