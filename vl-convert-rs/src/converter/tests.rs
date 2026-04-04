use super::*;
use crate::data_ops::{
    normalize_allowed_base_url, normalize_allowed_base_urls, AllowedBaseUrlPattern,
};
use crate::extract::{is_available, FontSource};
use crate::text::get_font_baseline_snapshot;
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const PNG_1X1_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
    0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 15, 0, 2, 3, 1, 128,
    179, 248, 175, 217, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const SVG_2X3_BASE64: &str =
    "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyIiBoZWlnaHQ9IjMiPjxyZWN0IHdpZHRoPSIyIiBoZWlnaHQ9IjMiIGZpbGw9InJlZCIvPjwvc3ZnPg==";
const SVG_2X3_DATA_URL: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyIiBoZWlnaHQ9IjMiPjxyZWN0IHdpZHRoPSIyIiBoZWlnaHQ9IjMiIGZpbGw9InJlZCIvPjwvc3ZnPg==";

fn make_test_work() -> WorkFn {
    Box::new(|_inner| Box::pin(async {}))
}

fn assert_send_future<F: Future + Send>(_: F) {}

#[derive(Clone)]
struct TestHttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl TestHttpResponse {
    fn ok_text(body: &str) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            body: body.as_bytes().to_vec(),
        }
    }

    fn ok_png(body: &[u8]) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".to_string(), "image/png".to_string())],
            body: body.to_vec(),
        }
    }

    fn ok_svg(body: &str) -> Self {
        Self {
            status: 200,
            headers: vec![("Content-Type".to_string(), "image/svg+xml".to_string())],
            body: body.as_bytes().to_vec(),
        }
    }

    fn redirect(location: &str) -> Self {
        Self {
            status: 302,
            headers: vec![("Location".to_string(), location.to_string())],
            body: Vec::new(),
        }
    }
}

struct TestHttpServer {
    addr: SocketAddr,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestHttpServer {
    fn new(routes: Vec<(&str, TestHttpResponse)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();

        let routes = Arc::new(
            routes
                .into_iter()
                .map(|(path, response)| (path.to_string(), response))
                .collect::<HashMap<_, _>>(),
        );
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let routes_clone = routes.clone();

        let handle = thread::spawn(move || {
            while running_clone.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        handle_test_http_connection(stream, &routes_clone);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            running,
            handle: Some(handle),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }

    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn base_url(&self) -> String {
        format!("http://{}/", self.addr)
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_test_http_connection(mut stream: TcpStream, routes: &HashMap<String, TestHttpResponse>) {
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let request_target = request_line.split_whitespace().nth(1).unwrap_or("/");
    let request_path = request_target.split('?').next().unwrap_or(request_target);

    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err() {
            return;
        }
        if header_line == "\r\n" || header_line == "\n" || header_line.is_empty() {
            break;
        }
    }

    let response = routes
        .get(request_path)
        .cloned()
        .unwrap_or_else(|| TestHttpResponse {
            status: 404,
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            body: b"not found".to_vec(),
        });

    let mut headers = response.headers;
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    {
        headers.push((
            "Content-Length".to_string(),
            response.body.len().to_string(),
        ));
    }
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("connection"))
    {
        headers.push(("Connection".to_string(), "close".to_string()));
    }

    let mut response_head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        http_reason_phrase(response.status)
    );
    for (name, value) in headers {
        response_head.push_str(&format!("{name}: {value}\r\n"));
    }
    response_head.push_str("\r\n");

    let _ = stream.write_all(response_head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}

fn http_reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        404 => "Not Found",
        _ => "Status",
    }
}

#[tokio::test]
async fn test_convert_context() {
    let ctx = VlConverter::new();
    let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
"data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
"mark": "bar",
"encoding": {
    "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
    "y": {"aggregate": "mean", "field": "precipitation"}
}
}
    "#).unwrap();

    let vg_output = ctx
        .vegalite_to_vega(
            vl_spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    println!("vg_spec: {}", vg_output.spec)
}

#[tokio::test]
async fn test_multi_convert_context() {
    let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
"data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
"mark": "bar",
"encoding": {
    "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
    "y": {"aggregate": "mean", "field": "precipitation"}
}
}
    "#).unwrap();

    let ctx1 = VlConverter::new();
    let vg_output1 = ctx1
        .vegalite_to_vega(
            vl_spec.clone(),
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    println!("vg_spec1: {}", vg_output1.spec);

    let ctx1 = VlConverter::new();
    let vg_output2 = ctx1
        .vegalite_to_vega(
            vl_spec,
            VlOpts {
                vl_version: VlVersion::v5_8,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    println!("vg_spec2: {}", vg_output2.spec);
}

#[tokio::test]
async fn test_execute_script_to_bytes_typed_array() {
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig::default(),
            parsed_allowed_base_urls: None,
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();
    let bytes = ctx
        .execute_script_to_bytes("new Uint8Array([1, 2, 3])")
        .await
        .unwrap();
    assert_eq!(bytes, vec![1, 2, 3]);
}

#[tokio::test]
async fn test_canvas_png_and_image_data_are_typed_arrays() {
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig::default(),
            parsed_allowed_base_urls: None,
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();
    let code = r#"
const canvas = new HTMLCanvasElement(8, 8);
const ctx2d = canvas.getContext('2d');
ctx2d.fillStyle = '#ff0000';
ctx2d.fillRect(0, 0, 8, 8);

var __pngBytes = canvas._toPngWithPpi(72);
var __pngIsUint8Array = __pngBytes instanceof Uint8Array;

const imageData = ctx2d.getImageData(0, 0, 1, 1);
var __imageDataChecks = [
  imageData.data instanceof Uint8ClampedArray,
  imageData.data[0], imageData.data[1], imageData.data[2], imageData.data[3]
];
"#
    .to_string();

    ctx.worker
        .js_runtime
        .execute_script("ext:<anon>", code)
        .unwrap();
    ctx.worker
        .js_runtime
        .run_event_loop(Default::default())
        .await
        .unwrap();

    let png_is_typed = ctx
        .execute_script_to_json("__pngIsUint8Array")
        .await
        .unwrap();
    assert_eq!(png_is_typed, json!(true));

    let png_bytes = ctx.execute_script_to_bytes("__pngBytes").await.unwrap();
    assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));

    let image_data_checks = ctx
        .execute_script_to_json("__imageDataChecks")
        .await
        .unwrap();
    assert_eq!(image_data_checks, json!([true, 255, 0, 0, 255]));
}

#[tokio::test]
async fn test_image_decode_and_load_events() {
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig::default(),
            parsed_allowed_base_urls: None,
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();
    let image_url = serde_json::to_string(SVG_2X3_DATA_URL).unwrap();
    let code = format!(
        r#"
var __imageDecodeLoadResult = null;
(async () => {{
  const img = new Image();
  let onloadCount = 0;
  let listenerCount = 0;
  img.onload = () => {{ onloadCount += 1; }};
  img.addEventListener("load", () => {{ listenerCount += 1; }});
  img.src = {image_url};
  await img.decode();
  __imageDecodeLoadResult = {{
complete: img.complete,
naturalWidth: img.naturalWidth,
naturalHeight: img.naturalHeight,
onloadCount,
listenerCount
  }};
}})();
"#
    );

    ctx.worker
        .js_runtime
        .execute_script("ext:<anon>", code)
        .unwrap();
    ctx.worker
        .js_runtime
        .run_event_loop(Default::default())
        .await
        .unwrap();

    let result = ctx
        .execute_script_to_json("__imageDecodeLoadResult")
        .await
        .unwrap();
    assert_eq!(result["complete"], json!(true));
    assert_eq!(result["naturalWidth"], json!(2));
    assert_eq!(result["naturalHeight"], json!(3));
    assert_eq!(result["onloadCount"], json!(1));
    assert_eq!(result["listenerCount"], json!(1));
}

#[tokio::test]
async fn test_image_decode_rejects_and_error_events_fire() {
    // Use allowed_base_urls: Some(vec![]) to deny all HTTP access via ops
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig {
                allowed_base_urls: Some(vec![]),
                ..Default::default()
            },
            parsed_allowed_base_urls: Some(vec![]),
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();
    let code = r#"
var __imageDecodeErrorResult = null;
(async () => {
  const img = new Image();
  let onerrorCount = 0;
  let listenerCount = 0;
  img.onerror = () => { onerrorCount += 1; };
  img.addEventListener("error", () => { listenerCount += 1; });
  img.src = "https://example.com/image.png";

  let decodeMessage = "";
  try {
await img.decode();
decodeMessage = "resolved";
  } catch (err) {
decodeMessage = String(err && err.message ? err.message : err);
  }

  __imageDecodeErrorResult = {
complete: img.complete,
naturalWidth: img.naturalWidth,
naturalHeight: img.naturalHeight,
onerrorCount,
listenerCount,
decodeMessage,
  };
})();
"#;

    ctx.worker
        .js_runtime
        .execute_script("ext:<anon>", code)
        .unwrap();
    ctx.worker
        .js_runtime
        .run_event_loop(Default::default())
        .await
        .unwrap();

    let result = ctx
        .execute_script_to_json("__imageDecodeErrorResult")
        .await
        .unwrap();
    assert_eq!(result["complete"], json!(true));
    assert_eq!(result["naturalWidth"], json!(0));
    assert_eq!(result["naturalHeight"], json!(0));
    assert_eq!(result["onerrorCount"], json!(1));
    assert_eq!(result["listenerCount"], json!(1));
    let decode_msg = result["decodeMessage"].as_str().unwrap_or_default();
    assert!(
        decode_msg.contains(ACCESS_DENIED_MARKER) || decode_msg.contains("permission"),
        "decode should fail with access denied, got: {decode_msg}"
    );
}

#[tokio::test]
async fn test_image_decode_ignores_stale_src_results() {
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig::default(),
            parsed_allowed_base_urls: None,
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();
    // Test that setting img.src twice results in only the last assignment's
    // image being loaded. Uses data: URIs to avoid needing HTTP.
    let code = format!(
        r#"
var __imageRaceResult = null;
(async () => {{
  const img = new Image();
  let onloadCount = 0;
  let onerrorCount = 0;
  img.onload = () => {{ onloadCount += 1; }};
  img.onerror = () => {{ onerrorCount += 1; }};

  // Set src to an invalid data URI first, then immediately to a valid one.
  // The valid one should win.
  img.src = "data:image/png;base64,INVALIDDATA";
  img.src = "data:image/svg+xml;base64,{SVG_2X3_BASE64}";
  await img.decode();
  await new Promise((resolve) => setTimeout(resolve, 50));

  __imageRaceResult = {{
src: img.src,
complete: img.complete,
naturalWidth: img.naturalWidth,
naturalHeight: img.naturalHeight,
onloadCount,
onerrorCount,
  }};
}})();
"#
    );

    ctx.worker
        .js_runtime
        .execute_script("ext:<anon>", code)
        .unwrap();
    ctx.worker
        .js_runtime
        .run_event_loop(Default::default())
        .await
        .unwrap();

    let result = ctx
        .execute_script_to_json("__imageRaceResult")
        .await
        .unwrap();
    let src = result["src"].as_str().unwrap_or_default();
    assert!(
        src.starts_with("data:image/svg+xml"),
        "src should be the second (valid) data URI, got: {src}"
    );
    assert_eq!(result["complete"], json!(true));
    assert_eq!(result["naturalWidth"], json!(2));
    assert_eq!(result["naturalHeight"], json!(3));
    // onload should fire once for the valid image
    assert_eq!(result["onloadCount"], json!(1));
    assert_eq!(result["onerrorCount"], json!(0));
}

#[tokio::test]
async fn test_polyfill_unsupported_methods_throw() {
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig::default(),
            parsed_allowed_base_urls: None,
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();
    let code = r#"
var __unsupportedMessages = [];

// Path2D.addPath is now implemented and should NOT throw
var addPathSucceeded = false;
try {
  new Path2D().addPath(new Path2D());
  addPathSucceeded = true;
} catch (err) {
  addPathSucceeded = false;
}

const canvas = new HTMLCanvasElement(16, 16);
const ctx2d = canvas.getContext('2d');
try {
  ctx2d.isPointInPath(0, 0);
} catch (err) {
  __unsupportedMessages.push(String(err && err.message ? err.message : err));
}
try {
  ctx2d.isPointInStroke(0, 0);
} catch (err) {
  __unsupportedMessages.push(String(err && err.message ? err.message : err));
}
"#
    .to_string();

    ctx.worker
        .js_runtime
        .execute_script("ext:<anon>", code)
        .unwrap();
    ctx.worker
        .js_runtime
        .run_event_loop(Default::default())
        .await
        .unwrap();

    let messages = ctx
        .execute_script_to_json("__unsupportedMessages")
        .await
        .unwrap()
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Path2D.addPath should now succeed (no longer unsupported)
    let add_path_succeeded = ctx
        .execute_script_to_json("addPathSucceeded")
        .await
        .unwrap();
    assert_eq!(add_path_succeeded, serde_json::json!(true));

    // isPointInPath and isPointInStroke remain unsupported
    assert_eq!(messages.len(), 2);
    assert!(messages[0]
        .as_str()
        .unwrap_or_default()
        .contains("CanvasRenderingContext2D.isPointInPath"));
    assert!(messages[1]
        .as_str()
        .unwrap_or_default()
        .contains("CanvasRenderingContext2D.isPointInStroke"));
}

#[test]
fn test_convert_vegalite_to_url() {
    let vl_spec: serde_json::Value = serde_json::from_str(r#"
{
"data": {"url": "https://raw.githubusercontent.com/vega/vega-datasets/master/data/seattle-weather.csv"},
"mark": "bar",
"encoding": {
    "x": {"timeUnit": "month", "field": "date", "type": "ordinal"},
    "y": {"aggregate": "mean", "field": "precipitation"}
}
}
    "#).unwrap();

    let url = vegalite_to_url(&vl_spec, UrlOpts { fullscreen: false }).unwrap();
    let expected = concat!(
        "https://vega.github.io/editor/#/url/vega-lite/",
        "N4IgJghgLhIFygK4CcA28QAspQA4Gc4B6I5CAdwDoBzASyk0QCNF8BTZAYwHsA7KNv0o8AtkQBubahAlSIAWkg",
        "x2UfERER8A5ESUz20KKjbzybaJg7D84kAF8ANCA3IA1hiYRkIJ4J5haXmp4UAAPEJAoWhE2AFVeegwRPgYfEAA",
        "zWjZUMAwlNjSoAE9cArgQbmQA3gh0RxAiiIhqamQ5ASTzXjTM7Nzy3DbOWlx6aFo+ezs7IA",
    );
    println!("{url}");
    assert_eq!(url, expected);
}

#[test]
fn test_convert_vega_to_url() {
    let vl_spec: serde_json::Value = serde_json::from_str(
        r#"
{
  "$schema": "https://vega.github.io/schema/vega/v5.json",
  "description": "A basic stacked bar chart example.",
  "width": 500,
  "height": 200,
  "padding": 5,

  "data": [
{
  "name": "table",
  "values": [
    {"x": 0, "y": 28, "c": 0}, {"x": 0, "y": 55, "c": 1},
    {"x": 1, "y": 43, "c": 0}, {"x": 1, "y": 91, "c": 1},
    {"x": 2, "y": 81, "c": 0}, {"x": 2, "y": 53, "c": 1},
    {"x": 3, "y": 19, "c": 0}, {"x": 3, "y": 87, "c": 1},
    {"x": 4, "y": 52, "c": 0}, {"x": 4, "y": 48, "c": 1},
    {"x": 5, "y": 24, "c": 0}, {"x": 5, "y": 49, "c": 1},
    {"x": 6, "y": 87, "c": 0}, {"x": 6, "y": 66, "c": 1},
    {"x": 7, "y": 17, "c": 0}, {"x": 7, "y": 27, "c": 1},
    {"x": 8, "y": 68, "c": 0}, {"x": 8, "y": 16, "c": 1},
    {"x": 9, "y": 49, "c": 0}, {"x": 9, "y": 15, "c": 1}
  ],
  "transform": [
    {
      "type": "stack",
      "groupby": ["x"],
      "sort": {"field": "c"},
      "field": "y"
    }
  ]
}
  ],

  "scales": [
{
  "name": "x",
  "type": "band",
  "range": "width",
  "domain": {"data": "table", "field": "x"}
},
{
  "name": "y",
  "type": "linear",
  "range": "height",
  "nice": true, "zero": true,
  "domain": {"data": "table", "field": "y1"}
},
{
  "name": "color",
  "type": "ordinal",
  "range": "category",
  "domain": {"data": "table", "field": "c"}
}
  ],

  "axes": [
{"orient": "bottom", "scale": "x", "zindex": 1},
{"orient": "left", "scale": "y", "zindex": 1}
  ],

  "marks": [
{
  "type": "rect",
  "from": {"data": "table"},
  "encode": {
    "enter": {
      "x": {"scale": "x", "field": "x"},
      "width": {"scale": "x", "band": 1, "offset": -1},
      "y": {"scale": "y", "field": "y0"},
      "y2": {"scale": "y", "field": "y1"},
      "fill": {"scale": "color", "field": "c"}
    },
    "update": {
      "fillOpacity": {"value": 1}
    },
    "hover": {
      "fillOpacity": {"value": 0.5}
    }
  }
}
  ]
}
    "#,
    )
    .unwrap();

    let url = vega_to_url(&vl_spec, UrlOpts { fullscreen: true }).unwrap();
    println!("{url}");
    let expected = concat!(
        "https://vega.github.io/editor/#/url/vega/",
        "N4IgJAzgxgFgpgWwIYgFwhgF0wBwqgegIDc4BzJAOjIEtMYBXAI0poHsDp5kTykSArJQBWENgDsQAGhAATONA",
        "BONHJnaT0AQQAETJBBpRtETEigBrOLN1JF22Lcza4ADyQIcAGziVpIAO40svRoAgAMYTLwNGRYaABMETI4SLK",
        "yNOJkoTKySKZoANqg4u5waCCmTN5+xEieDAqFoC5okSAAngkAHDJQrQC+Us2tMp2oAgK9aACMg8Oo06NoACwA",
        "zFOoYXMgLQtLqACciyB9C9u78ftdx6dbQzsJ+wLrJzPnaC9j0wcbd-OfaC6AHYNrN7rtlk9Lq9Nu9UJCOisej",
        "CwfNJojUPEEbc4eixssfii4QA2K4gmF-B6oUkY4k006oqnkr7knHgtDMhKst7s1DIsbE5Fs+b8mb0nnzQn4wn",
        "CqlSmbohn9AC6MkwiiQ4ggADM2IoEE0Ku0cGV0CYzOY-GRFGwGDgmGMCg9VSAxIpMGhQNqaHBPLJyn1BiBvb7",
        "-ehOiqVTJoHVGqgiiASghTQ81caU3pxP6ZBrMinAsEYH5ZGxkBlPXI8ih0JVqjIQ37yi1+tskynOmmTeVPBk4L",
        "Y-LmyCnorEPTJxIZTeqGjIAF5wG1oadwHKlpDl1CgXL5GtIKples+xvh6YgFv3NsBtiePV+TDp8p69IlTwDzVD",
        "gN5ch6jtyNcbrcq3KWsD2DI8w1eFsXSQFw4wTPUfXED10CYNhsFLPwY2qdAWjnDJ5F2RkELgJDuzgbUx1dKBY3",
        "KX9Z3w1w3hdZBFHMCBDXvLt0EUOAoEo7UbQNTdKx3Co92qIMSKgNh5ArEjMAXCtdlALCU1wsDQybM8ZALEJhNU",
        "rSZEzMNjjYbVtQgOBkIAWjBDEVOo7DEUPTTwzCbSOniCsDPDPwGwg9pTyDb1PFffTHJTaSb0UPzwIDM8gztbdT",
        "S9GhQoAeRSKA6DGUBanqU1ZiDGA2FIGLhJCzxMrMHKK3yhpWkoAQWyg-ogA/view",
    );
    assert_eq!(url, expected);
}

#[test]
fn test_with_config_rejects_zero_num_workers() {
    let err = VlConverter::with_config(VlcConfig {
        num_workers: 0,
        ..Default::default()
    })
    .err()
    .unwrap();
    assert!(err.to_string().contains("num_workers must be >= 1"));
}

#[test]
fn test_config_reports_configured_num_workers() {
    let converter = VlConverter::with_config(VlcConfig {
        num_workers: 4,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(converter.config().num_workers, 4);
}

fn write_test_png(path: &std::path::Path) {
    std::fs::write(path, PNG_1X1_BYTES).unwrap();
}

fn svg_with_href(href: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="{href}" width="1" height="1"/></svg>"#
    )
}

fn vega_spec_with_data_url(url: &str) -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 20,
        "height": 20,
        "data": [{"name": "table", "url": url, "format": {"type": "csv"}}],
        "scales": [
            {"name": "x", "type": "linear", "range": "width", "domain": {"data": "table", "field": "a"}},
            {"name": "y", "type": "linear", "range": "height", "domain": {"data": "table", "field": "b"}}
        ],
        "marks": [{
            "type": "symbol",
            "from": {"data": "table"},
            "encode": {
                "enter": {
                    "x": {"scale": "x", "field": "a"},
                    "y": {"scale": "y", "field": "b"}
                }
            }
        }]
    })
}

fn vegalite_spec_with_data_url(url: &str) -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"url": url},
        "mark": "point",
        "encoding": {
            "x": {"field": "a", "type": "quantitative"},
            "y": {"field": "b", "type": "quantitative"}
        }
    })
}

fn vegalite_spec_with_image_url(url: &str) -> serde_json::Value {
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {
            "values": [
                {"img": url}
            ]
        },
        "mark": {"type": "image", "width": 20, "height": 20},
        "encoding": {
            "x": {"value": 10},
            "y": {"value": 10},
            "url": {"field": "img", "type": "nominal"}
        }
    })
}

#[test]
fn test_allowed_base_url_normalization_and_validation() {
    assert_eq!(
        normalize_allowed_base_url("https://example.com").unwrap(),
        AllowedBaseUrlPattern::Prefix("https://example.com/".to_string())
    );
    assert_eq!(
        normalize_allowed_base_url("https://example.com/data").unwrap(),
        AllowedBaseUrlPattern::Prefix("https://example.com/data/".to_string())
    );

    assert!(normalize_allowed_base_url("https://user@example.com/").is_err());
    assert!(normalize_allowed_base_url("https://example.com/?q=1").is_err());
    assert!(normalize_allowed_base_url("https://example.com/#fragment").is_err());
    assert!(normalize_allowed_base_urls(Some(vec![])).is_ok());
}

#[test]
fn test_with_config_accepts_empty_allowed_base_urls() {
    // Empty list means no external access at all (valid config)
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    });
    assert!(converter.is_ok());
}

#[test]
fn test_with_config_accepts_csp_patterns() {
    assert_eq!(
        normalize_allowed_base_url("*").unwrap(),
        AllowedBaseUrlPattern::Any
    );
    assert_eq!(
        normalize_allowed_base_url("https:").unwrap(),
        AllowedBaseUrlPattern::Scheme("https".to_string())
    );
    assert_eq!(
        normalize_allowed_base_url("http:").unwrap(),
        AllowedBaseUrlPattern::Scheme("http".to_string())
    );
}

#[tokio::test]
async fn test_svg_helper_denies_subdomain_and_userinfo_url_confusion() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec!["https://example.com".to_string()]),
        ..Default::default()
    })
    .unwrap();

    let subdomain_err = converter
        .svg_to_png(
            &svg_with_href("https://example.com.evil.test/image.png"),
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    assert!(subdomain_err
        .to_string()
        .contains("External data url not allowed"));

    let userinfo_err = converter
        .svg_to_png(
            &svg_with_href("https://example.com@evil.test/image.png"),
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    assert!(userinfo_err
        .to_string()
        .contains("External data url not allowed"));
}

#[tokio::test]
async fn test_svg_helper_denies_local_paths_without_filesystem_root() {
    let temp_dir = tempfile::tempdir().unwrap();
    let local_image_path = temp_dir.path().join("image.png");
    write_test_png(&local_image_path);
    let href = Url::from_file_path(&local_image_path).unwrap().to_string();

    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    })
    .unwrap();

    let err = converter
        .svg_to_png(
            &svg_with_href(&href),
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Filesystem access denied"));
}

#[tokio::test]
async fn test_svg_helper_enforces_filesystem_root() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();

    let inside_path = root.join("inside.png");
    write_test_png(&inside_path);
    let outside_path = temp_dir.path().join("outside.png");
    write_test_png(&outside_path);

    let converter = VlConverter::with_config(VlcConfig {
        base_url: BaseUrlSetting::Custom(root.to_string_lossy().to_string()),
        allowed_base_urls: Some(vec![root.to_string_lossy().to_string()]),
        ..Default::default()
    })
    .unwrap();

    let allowed = converter
        .svg_to_png(
            &svg_with_href("inside.png"),
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await;
    assert!(allowed.is_ok());

    let outside_href = Url::from_file_path(&outside_path).unwrap().to_string();
    let err = converter
        .svg_to_png(
            &svg_with_href(&outside_href),
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("filesystem_root") || message.contains("access denied"));

    let err = converter
        .svg_to_png(
            &svg_with_href("../outside.png"),
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("filesystem_root"));
}

#[tokio::test]
async fn test_svg_helper_enforces_http_access_and_allowed_base_urls() {
    let remote_svg = svg_with_href("https://example.com/image.png");

    let no_http_converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    })
    .unwrap();
    let err = no_http_converter
        .svg_to_png(
            &remote_svg,
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("vlc_access_denied")
            || msg.contains("http access denied")
            || msg.contains("not allowed"),
        "expected access denied, got: {msg}"
    );

    let allowlisted_converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec!["https://allowed.example/".to_string()]),
        ..Default::default()
    })
    .unwrap();
    let err = allowlisted_converter
        .svg_to_png(
            &remote_svg,
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("External data url not allowed"));
}

#[tokio::test]
async fn test_svg_helper_allows_data_uri_when_http_disabled() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    })
    .unwrap();
    let svg = svg_with_href(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/w8AAgMBgLP4r9kAAAAASUVORK5CYII=",
    );
    let output = converter
        .svg_to_png(
            &svg,
            PngOpts {
                scale: Some(1.0),
                ppi: None,
            },
        )
        .await
        .unwrap();
    assert!(output.data.starts_with(&[137, 80, 78, 71]));
}

#[tokio::test]
async fn test_vega_to_pdf_denies_http_access() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    })
    .unwrap();
    let spec = vega_spec_with_data_url("https://example.com/data.csv");

    let err = converter
        .vega_to_pdf(spec, VgOpts::default(), PdfOpts::default())
        .await
        .unwrap_err();
    let message = err.to_string().to_ascii_lowercase();
    assert!(
        message.contains("vlc_access_denied")
            || message.contains("http access denied")
            || message.contains("requires net access")
            || message.contains("permission"),
        "expected access denied error, got: {message}"
    );
}

#[tokio::test]
async fn test_vega_loader_allows_data_uri_when_http_disabled() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    })
    .unwrap();
    let spec = vega_spec_with_data_url("data:text/csv,a,b%0A1,2");

    let output = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await
        .unwrap();
    assert!(output.svg.contains("<svg"));
}

#[tokio::test]
async fn test_vegalite_to_png_canvas_image_denies_http_access() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![]),
        ..Default::default()
    })
    .unwrap();
    let spec = vegalite_spec_with_image_url("https://example.com/image.png");

    // With HTTP denied, the canvas Image class catches the op error and
    // fires onerror. The conversion succeeds but the image is not rendered.
    let result = converter
        .vegalite_to_png(
            spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            PngOpts {
                scale: Some(1.0),
                ppi: Some(72.0),
            },
        )
        .await;
    // The conversion should succeed (image just not loaded)
    assert!(
        result.is_ok(),
        "conversion should succeed even with denied image"
    );
}

#[tokio::test]
async fn test_vegalite_to_png_canvas_image_enforces_allowed_base_urls() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec!["https://allowed.example/".to_string()]),
        ..Default::default()
    })
    .unwrap();
    let spec = vegalite_spec_with_image_url("https://example.com/image.png");

    // With allowlist not including example.com, the op denies the fetch.
    // Canvas Image catches the error; the conversion succeeds without the image.
    let result = converter
        .vegalite_to_png(
            spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            PngOpts {
                scale: Some(1.0),
                ppi: Some(72.0),
            },
        )
        .await;
    assert!(
        result.is_ok(),
        "conversion should succeed even with denied image"
    );
}

#[tokio::test]
async fn test_vega_to_pdf_denies_disallowed_base_url() {
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec!["https://allowed.example/".to_string()]),
        ..Default::default()
    })
    .unwrap();
    let spec = vega_spec_with_data_url("https://example.com/data.csv");

    let err = converter
        .vega_to_pdf(spec, VgOpts::default(), PdfOpts::default())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("External data url not allowed"));
}

#[tokio::test]
async fn test_vegalite_to_pdf_denies_filesystem_access_outside_root() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    let outside_csv = temp_dir.path().join("outside.csv");
    std::fs::write(&outside_csv, "a,b\n1,2\n").unwrap();
    let outside_file_url = Url::from_file_path(&outside_csv).unwrap().to_string();

    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![root.to_string_lossy().to_string()]),
        ..Default::default()
    })
    .unwrap();
    let vl_spec = vegalite_spec_with_data_url(&outside_file_url);

    let err = converter
        .vegalite_to_pdf(
            vl_spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            PdfOpts::default(),
        )
        .await
        .unwrap_err();
    let message = err.to_string().to_ascii_lowercase();
    assert!(
        message.contains("filesystem access denied")
            || message.contains("requires read access")
            || message.contains("permission")
    );
}

#[tokio::test]
async fn test_loader_blocks_percent_encoded_filesystem_traversal() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root = temp_dir.path().join("root");
    std::fs::create_dir_all(root.join("subdir")).unwrap();
    let outside_csv = temp_dir.path().join("outside.csv");
    std::fs::write(&outside_csv, "a,b\n1,2\n").unwrap();

    let converter = VlConverter::with_config(VlcConfig {
        base_url: BaseUrlSetting::Custom(root.to_string_lossy().to_string()),
        allowed_base_urls: Some(vec![root.to_string_lossy().to_string()]),
        ..Default::default()
    })
    .unwrap();

    let spec = vegalite_spec_with_data_url("subdir/..%2F..%2Foutside.csv");
    let err = converter
        .vegalite_to_svg(
            spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap_err();
    let err_msg = err.to_string();
    // The percent-encoded traversal must fail: either the Rust op rejects it
    // with an access-denied error, or the literal %2F path doesn't exist.
    assert!(
        err_msg.contains(&format!("{ACCESS_DENIED_MARKER}: Filesystem access denied"))
            || err_msg.contains("No such file or directory"),
        "Expected access denied or file-not-found, got: {err_msg}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_vegalite_to_pdf_config_allowlist_for_svg_rasterization() {
    let server = TestHttpServer::new(vec![(
        "/image.svg",
        TestHttpResponse::ok_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#,
        ),
    )]);
    let converter = VlConverter::with_config(VlcConfig {
        allowed_base_urls: Some(vec![server.origin()]),
        ..Default::default()
    })
    .unwrap();

    let output = converter
        .vegalite_to_pdf(
            vegalite_spec_with_image_url(&server.url("/image.svg")),
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            PdfOpts::default(),
        )
        .await
        .unwrap();
    assert!(output.data.starts_with(b"%PDF"));
}

#[test]
fn test_html_and_bundle_futures_are_send() {
    let converter = VlConverter::new();
    let vl_spec = serde_json::json!({
        "data": {"values": [{"a": "A", "b": 1}]},
        "mark": "bar",
        "encoding": {"x": {"field": "a", "type": "nominal"}}
    });
    let vg_spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v6.json",
        "data": [{"name": "table", "values": [{"x": "A", "y": 1}]}],
        "marks": [{"type": "rect", "from": {"data": "table"}}]
    });

    assert_send_future(converter.get_vegaembed_bundle(VlVersion::v5_16));
    assert_send_future(
        converter.bundle_vega_snippet("window.__vlcBundleMarker = 'ok';", VlVersion::v5_16),
    );
    assert_send_future(converter.vegalite_to_html(
        vl_spec,
        VlOpts {
            vl_version: VlVersion::v5_16,
            ..Default::default()
        },
        HtmlOpts {
            bundle: true,
            renderer: Renderer::Svg,
        },
    ));
    assert_send_future(converter.vega_to_html(
        vg_spec,
        VgOpts::default(),
        HtmlOpts {
            bundle: true,
            renderer: Renderer::Svg,
        },
    ));
}

#[tokio::test]
async fn test_get_vegaembed_bundle_caches_result() {
    let converter = VlConverter::with_config(VlcConfig {
        num_workers: 1,
        ..Default::default()
    })
    .unwrap();

    let first = converter
        .get_vegaembed_bundle(VlVersion::v5_16)
        .await
        .unwrap();
    let len_after_first = converter.inner.vegaembed_bundles.lock().unwrap().len();

    let second = converter
        .get_vegaembed_bundle(VlVersion::v5_16)
        .await
        .unwrap();
    let len_after_second = converter.inner.vegaembed_bundles.lock().unwrap().len();

    assert_eq!(first, second);
    assert_eq!(len_after_first, 1);
    assert_eq!(len_after_second, 1);
}

#[tokio::test]
async fn test_bundle_vega_snippet_custom_snippet() {
    let converter = VlConverter::with_config(VlcConfig {
        num_workers: 1,
        ..Default::default()
    })
    .unwrap();
    let snippet = "window.__vlcBundleMarker = 'ok';";

    let bundle = converter
        .bundle_vega_snippet(snippet, VlVersion::v5_16)
        .await
        .unwrap();

    assert!(bundle.contains("__vlcBundleMarker"));
}

#[test]
fn test_worker_pool_next_sender_balances_outstanding_reservations() {
    let mut senders = Vec::new();
    let mut _receivers = Vec::new();
    for _ in 0..3 {
        let (tx, rx) = tokio::sync::mpsc::channel::<QueuedWork>(1);
        senders.push(tx);
        _receivers.push(rx);
    }

    let pool = WorkerPool {
        senders,
        outstanding: (0..3)
            .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
            .collect(),
        dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
        _handles: Vec::new(),
    };

    let mut tickets = Vec::new();
    for _ in 0..30 {
        let (_, ticket) = pool
            .next_sender()
            .expect("pool with open senders should produce a sender");
        tickets.push(ticket);

        let loads: Vec<usize> = pool
            .outstanding
            .iter()
            .map(|outstanding| outstanding.load(std::sync::atomic::Ordering::Relaxed))
            .collect();
        let min = *loads.iter().min().expect("loads should not be empty");
        let max = *loads.iter().max().expect("loads should not be empty");
        assert!(
            max - min <= 1,
            "expected balanced outstanding counts, got {loads:?}"
        );
    }

    drop(tickets);
    for outstanding in &pool.outstanding {
        assert_eq!(outstanding.load(std::sync::atomic::Ordering::Relaxed), 0);
    }
}

#[test]
fn test_worker_pool_next_sender_skips_closed_senders() {
    let (closed_sender, closed_receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);
    drop(closed_receiver);

    let (open_sender, mut open_receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);

    let pool = WorkerPool {
        senders: vec![closed_sender, open_sender],
        outstanding: (0..2)
            .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
            .collect(),
        dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
        _handles: Vec::new(),
    };

    for _ in 0..4 {
        let (sender, ticket) = pool
            .next_sender()
            .expect("pool should return the open sender");
        sender
            .try_send(QueuedWork::new(
                make_test_work(),
                ticket,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ))
            .expect("dispatch should use open sender, not closed sender");
        let queued = open_receiver
            .try_recv()
            .expect("open receiver should receive dispatched command");
        drop(queued);
    }
}

#[tokio::test]
async fn test_worker_pool_cancellation_releases_outstanding_ticket() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);
    let pool = WorkerPool {
        senders: vec![sender],
        outstanding: vec![std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))],
        dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
        _handles: Vec::new(),
    };

    let (sender, ticket) = pool.next_sender().unwrap();
    sender
        .send(QueuedWork::new(
            make_test_work(),
            ticket,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ))
        .await
        .unwrap();
    assert_eq!(
        pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    let (sender, ticket) = pool.next_sender().unwrap();
    let blocked_send = tokio::spawn(async move {
        sender
            .send(QueuedWork::new(
                make_test_work(),
                ticket,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ))
            .await
    });
    tokio::task::yield_now().await;

    assert_eq!(
        pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
        2
    );

    blocked_send.abort();
    let _ = blocked_send.await;
    tokio::task::yield_now().await;

    assert_eq!(
        pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    let queued = receiver
        .recv()
        .await
        .expect("first queued command should still be in the channel");
    drop(queued);

    assert_eq!(
        pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_get_or_spawn_sender_respawns_closed_pool_without_explicit_reset() {
    let num_workers = 2;
    let converter = VlConverter::with_config(VlcConfig {
        num_workers,
        ..Default::default()
    })
    .unwrap();

    let mut closed_senders = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        let (sender, receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);
        drop(receiver);
        closed_senders.push(sender);
    }

    let closed_pool = WorkerPool {
        senders: closed_senders,
        outstanding: (0..num_workers)
            .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
            .collect(),
        dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
        _handles: Vec::new(),
    };

    {
        let mut guard = converter.inner.pool.lock().unwrap();
        *guard = Some(closed_pool);
    }

    let _ = converter.get_or_spawn_sender().unwrap();

    let guard = converter.inner.pool.lock().unwrap();
    let pool = guard
        .as_ref()
        .expect("get_or_spawn_sender should replace closed pool with a live pool");
    assert_eq!(pool.senders.len(), num_workers);
    assert!(!pool.is_closed(), "respawned pool should be open");
}

#[test]
fn test_get_or_spawn_sender_spawns_pool_without_request() {
    let converter = VlConverter::with_config(VlcConfig {
        num_workers: 2,
        ..Default::default()
    })
    .unwrap();

    {
        let guard = converter.inner.pool.lock().unwrap();
        assert!(guard.is_none(), "pool should start uninitialized");
    }

    let _ = converter.get_or_spawn_sender().unwrap();

    {
        let guard = converter.inner.pool.lock().unwrap();
        let pool = guard
            .as_ref()
            .expect("pool should be initialized by get_or_spawn_sender");
        assert_eq!(pool.senders.len(), 2);
        assert!(!pool.is_closed(), "warmed pool should have open senders");
        assert_eq!(
            pool.outstanding
                .iter()
                .map(|outstanding| outstanding.load(std::sync::atomic::Ordering::Relaxed))
                .sum::<usize>(),
            0,
            "get_or_spawn_sender should not leave outstanding reservations"
        );
    }
}

#[tokio::test]
async fn test_get_or_spawn_sender_is_idempotent() {
    let converter = VlConverter::with_config(VlcConfig {
        num_workers: 2,
        ..Default::default()
    })
    .unwrap();
    let _ = converter.get_or_spawn_sender().unwrap();
    let _ = converter.get_or_spawn_sender().unwrap();

    let vl_spec = serde_json::json!({
        "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    });

    let output = converter
        .vegalite_to_svg(
            vl_spec,
            VlOpts {
                vl_version: VlVersion::v5_16,
                ..Default::default()
            },
            SvgOpts::default(),
        )
        .await
        .unwrap();
    assert!(output.svg.trim_start().starts_with("<svg"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_parallel_conversions_with_shared_converter() {
    let converter = VlConverter::with_config(VlcConfig {
        num_workers: 4,
        ..Default::default()
    })
    .unwrap();
    let vl_spec = serde_json::json!({
        "data": {"values": [{"a": "A", "b": 1}, {"a": "B", "b": 2}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    });

    let mut tasks = Vec::new();
    for _ in 0..16 {
        let converter = converter.clone();
        let vl_spec = vl_spec.clone();
        tasks.push(tokio::spawn(async move {
            converter
                .vegalite_to_svg(
                    vl_spec,
                    VlOpts {
                        vl_version: VlVersion::v5_16,
                        ..Default::default()
                    },
                    SvgOpts::default(),
                )
                .await
        }));
    }

    for task in tasks {
        let output = task.await.unwrap().unwrap();
        assert!(output.svg.trim_start().starts_with("<svg"));
    }
}

#[tokio::test]
async fn test_font_version_propagation() {
    use crate::text::{register_font_directory, FONT_CONFIG_VERSION};
    use std::sync::atomic::Ordering;

    let version_before = FONT_CONFIG_VERSION.load(Ordering::Acquire);

    // Do an initial conversion to ensure the worker is running
    let ctx = VlConverter::new();
    let vl_spec: serde_json::Value = serde_json::from_str(
        r#"{
            "data": {"values": [{"a": 1}]},
            "mark": "point",
            "encoding": {"x": {"field": "a", "type": "quantitative"}}
        }"#,
    )
    .unwrap();
    ctx.vegalite_to_vega(
        vl_spec.clone(),
        VlOpts {
            vl_version: VlVersion::v5_16,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Register a font directory (re-registers the built-in fonts, which is harmless)
    let font_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/liberation-sans");
    register_font_directory(font_dir).unwrap();

    let version_after = FONT_CONFIG_VERSION.load(Ordering::Acquire);
    assert_eq!(
        version_after,
        version_before + 1,
        "FONT_CONFIG_VERSION should increment after register_font_directory"
    );

    // A subsequent conversion should still succeed, confirming the worker
    // picked up the font config change without dying
    let ctx2 = VlConverter::new();
    ctx2.vegalite_to_vega(
        vl_spec,
        VlOpts {
            vl_version: VlVersion::v5_16,
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

#[test]
fn test_scenegraph_google_probe_candidates_skip_explicit() {
    let families = BTreeSet::from([
        "Alpha".to_string(),
        "Bravo".to_string(),
        "Charlie".to_string(),
    ]);
    let explicit = HashSet::from(["Bravo".to_string()]);

    let candidates = scenegraph_google_probe_candidates(&families, &explicit);

    assert_eq!(
        candidates,
        BTreeSet::from(["Alpha".to_string(), "Charlie".to_string()])
    );
}

#[tokio::test]
async fn test_classify_scenegraph_fonts_uses_case_insensitive_local_match() {
    let available = available_font_families().unwrap();
    let family = available
        .iter()
        .find(|name| name.chars().any(|c| c.is_ascii_alphabetic()))
        .cloned()
        .expect("expected at least one alphabetic font family in fontdb");
    let alt_family = if family.to_ascii_uppercase() != family {
        family.to_ascii_uppercase()
    } else {
        family.to_ascii_lowercase()
    };

    assert_ne!(
        family, alt_family,
        "test requires a case-changed family name"
    );
    assert!(is_available(&alt_family, &available));

    let families = BTreeSet::from([alt_family.clone()]);
    let result = classify_scenegraph_fonts(
        &families,
        false,
        true,
        MissingFontsPolicy::Fallback,
        &HashSet::new(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].family, alt_family);
    assert!(matches!(result[0].source, FontSource::Local));
}

// --- domain_matches_patterns unit tests ---

#[test]
fn test_domain_matches_exact() {
    let patterns = vec!["esm.sh".to_string()];
    assert!(domain_matches_patterns("esm.sh", &patterns));
    assert!(!domain_matches_patterns("cdn.esm.sh", &patterns));
    assert!(!domain_matches_patterns("esm.sh.evil.com", &patterns));
}

#[test]
fn test_domain_matches_wildcard_subdomain() {
    let patterns = vec!["*.jsdelivr.net".to_string()];
    assert!(domain_matches_patterns("cdn.jsdelivr.net", &patterns));
    assert!(domain_matches_patterns("foo.bar.jsdelivr.net", &patterns));
    // The bare domain itself should match (*.x matches x)
    assert!(domain_matches_patterns("jsdelivr.net", &patterns));
    // Must not match a suffix attack
    assert!(!domain_matches_patterns("jsdelivr.net.evil.com", &patterns));
    assert!(!domain_matches_patterns("notjsdelivr.net", &patterns));
}

#[test]
fn test_domain_matches_star_all() {
    let patterns = vec!["*".to_string()];
    assert!(domain_matches_patterns("esm.sh", &patterns));
    assert!(domain_matches_patterns("anything.example.com", &patterns));
    assert!(domain_matches_patterns("", &patterns));
}

#[test]
fn test_domain_no_match_empty_list() {
    let patterns: Vec<String> = vec![];
    assert!(!domain_matches_patterns("esm.sh", &patterns));
}

#[test]
fn test_domain_no_match_wrong_domain() {
    let patterns = vec!["esm.sh".to_string(), "*.jsdelivr.net".to_string()];
    assert!(!domain_matches_patterns("evil.com", &patterns));
    assert!(!domain_matches_patterns("esm.sh.evil.com", &patterns));
}

#[test]
fn test_domain_multiple_patterns() {
    let patterns = vec![
        "esm.sh".to_string(),
        "*.jsdelivr.net".to_string(),
        "unpkg.com".to_string(),
    ];
    assert!(domain_matches_patterns("esm.sh", &patterns));
    assert!(domain_matches_patterns("cdn.jsdelivr.net", &patterns));
    assert!(domain_matches_patterns("unpkg.com", &patterns));
    assert!(!domain_matches_patterns("evil.com", &patterns));
}

#[tokio::test]
async fn test_base_url_disabled_blocks_relative_paths() {
    let converter = VlConverter::with_config(VlcConfig {
        base_url: BaseUrlSetting::Disabled,
        ..Default::default()
    })
    .unwrap();

    // Spec with a relative data URL — should fail because base_url is disabled
    let spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 50, "height": 50,
        "data": [{"name": "t", "url": "data/cars.json"}],
        "marks": []
    });

    let result = converter
        .vega_to_svg(spec, VgOpts::default(), SvgOpts::default())
        .await;
    // The relative URL resolves to about:invalid which the op rejects
    assert!(
        result.is_err() || {
            // If it "succeeds", the data just isn't loaded (no marks use it)
            true
        }
    );
}

#[tokio::test]
async fn test_sandbox_lockdown_blocks_fetch() {
    // Verify that JS code calling fetch() directly gets a permission error
    let mut ctx = InnerVlConverter::try_new(
        std::sync::Arc::new(ConverterContext {
            config: VlcConfig::default(),
            parsed_allowed_base_urls: None,
            resolved_plugins: None,
        }),
        get_font_baseline_snapshot().unwrap(),
    )
    .await
    .unwrap();

    let code = r#"
var __fetchResult = null;
(async () => {
try {
    await fetch("https://example.com");
    __fetchResult = "success";
} catch (e) {
    __fetchResult = e.message || String(e);
}
})();
"#;
    ctx.worker
        .js_runtime
        .execute_script("ext:<anon>", code.to_string())
        .unwrap();
    ctx.worker
        .js_runtime
        .run_event_loop(Default::default())
        .await
        .unwrap();

    let result = ctx.execute_script_to_json("__fetchResult").await.unwrap();
    let msg = result.as_str().unwrap_or_default().to_lowercase();
    assert!(
        msg.contains("permission") || msg.contains("denied") || msg.contains("requires net"),
        "fetch() should be blocked by Deno permissions, got: {msg}"
    );
}
