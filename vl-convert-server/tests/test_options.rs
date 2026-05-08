mod common;

use common::*;

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..4], &[0x89, b'P', b'N', b'G'], "bad PNG magic");
    assert_eq!(&bytes[12..16], b"IHDR", "missing PNG IHDR chunk");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (width, height)
}

fn svg_root_numeric_attr(svg: &str, attr: &str) -> f64 {
    let root_end = svg.find('>').expect("SVG root tag must close");
    let root = &svg[..root_end];
    let prefix = format!("{attr}=\"");
    let start = root
        .find(&prefix)
        .unwrap_or_else(|| panic!("SVG root missing {attr:?}: {root}"))
        + prefix.len();
    let end = root[start..]
        .find('"')
        .unwrap_or_else(|| panic!("SVG root {attr:?} value is unterminated: {root}"));
    root[start..start + end]
        .trim_end_matches("px")
        .parse()
        .unwrap_or_else(|err| panic!("SVG root {attr:?} is not numeric: {err}; root={root}"))
}

#[tokio::test]
async fn test_png_scale() {
    let server = &*DEFAULT_SERVER;
    let resp1 = server
        .client
        .post(format!("{}/vegalite/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);
    let bytes1 = resp1.bytes().await.unwrap();

    let resp2 = server
        .client
        .post(format!("{}/vegalite/png", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "scale": 2.0}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200);
    let bytes2 = resp2.bytes().await.unwrap();

    let (w1, h1) = png_dimensions(&bytes1);
    let (w2, h2) = png_dimensions(&bytes2);
    assert_eq!((w2, h2), (w1 * 2, h1 * 2));
}

#[tokio::test]
async fn test_jpeg_quality() {
    let server = &*DEFAULT_SERVER;
    let resp_low = server
        .client
        .post(format!("{}/vegalite/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "quality": 1}))
        .send()
        .await
        .unwrap();
    let bytes_low = resp_low.bytes().await.unwrap();

    let resp_high = server
        .client
        .post(format!("{}/vegalite/jpeg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "quality": 100}))
        .send()
        .await
        .unwrap();
    let bytes_high = resp_high.bytes().await.unwrap();

    assert!(
        bytes_high.len() > bytes_low.len(),
        "quality 100 should be larger than quality 1: {} vs {}",
        bytes_high.len(),
        bytes_low.len()
    );
}

#[tokio::test]
async fn test_html_renderer_canvas() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/html", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "renderer": "canvas"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("canvas"),
        "expected 'canvas' renderer in HTML output"
    );
}

#[tokio::test]
async fn test_url_fullscreen() {
    let server = &*DEFAULT_SERVER;
    let resp = server
        .client
        .post(format!("{}/vegalite/url", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "fullscreen": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("/view"),
        "expected /view in fullscreen URL, got: {}",
        &body[..body.len().min(100)]
    );
}

#[tokio::test]
async fn test_width_override() {
    let server = &*DEFAULT_SERVER;
    let default_resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec()}))
        .send()
        .await
        .unwrap();
    assert_eq!(default_resp.status(), 200);
    let default_body = default_resp.text().await.unwrap();

    let override_resp = server
        .client
        .post(format!("{}/vegalite/svg", server.base_url))
        .json(&serde_json::json!({"spec": simple_vl_spec(), "width": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(override_resp.status(), 200);
    let override_body = override_resp.text().await.unwrap();
    assert!(default_body.starts_with("<svg"), "expected SVG output");
    assert!(override_body.starts_with("<svg"), "expected SVG output");

    let default_width = svg_root_numeric_attr(&default_body, "width");
    let override_width = svg_root_numeric_attr(&override_body, "width");
    assert!(
        override_width > default_width,
        "width override should increase SVG root width; default={default_width}, override={override_width}"
    );
}
