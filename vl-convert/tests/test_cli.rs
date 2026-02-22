// Allow deprecated APIs (assert_cmd::cargo_bin is deprecated but still works)
#![allow(deprecated)]

use assert_cmd::prelude::*; // Add methods on commands
use dssim::{Dssim, DssimImage};
use predicates::prelude::*; // Used for writing assertions
use rstest::rstest;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::str::FromStr; // Run programs
use std::sync::Once;
use tempfile::NamedTempFile;
use vl_convert_rs::VlVersion;

const BACKGROUND_COLOR: &str = "#abc";
static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let outdir = root_path.join("tests").join("output");
        fs::remove_dir_all(&outdir).ok();
        fs::create_dir_all(&outdir).unwrap();
    });
}

#[rustfmt::skip]
mod test_access_flags {
    use crate::*;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    const PNG_1X1: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
        8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 15,
        0, 2, 3, 1, 128, 179, 248, 175, 217, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[derive(Clone)]
    struct TestHttpResponse {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl TestHttpResponse {
        fn ok_csv(body: &str) -> Self {
            Self {
                status: 200,
                headers: vec![("Content-Type".to_string(), "text/csv".to_string())],
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

    fn handle_test_http_connection(
        mut stream: TcpStream,
        routes: &HashMap<String, TestHttpResponse>,
    ) {
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
            headers.push(("Content-Length".to_string(), response.body.len().to_string()));
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
            302 => "Found",
            404 => "Not Found",
            _ => "Status",
        }
    }

    fn file_href(path: &std::path::Path) -> String {
        let absolute = path.canonicalize().unwrap();
        if cfg!(target_family = "windows") {
            format!("file:///{}", absolute.to_string_lossy().replace('\\', "/"))
        } else {
            format!("file://{}", absolute.to_string_lossy())
        }
    }

    #[test]
    fn test_vl2svg_denied_http_access() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output = output_path("access_vl2svg.svg");
        let mut cmd = Command::cargo_bin("vl-convert")?;
        cmd.arg("vl2svg")
            .arg("-i").arg(vl_spec_path("seattle-weather"))
            .arg("-o").arg(&output)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--no-http-access")
            .assert()
            .failure()
            .stderr(predicate::str::contains("access").or(predicate::str::contains("denied")));

        Ok(())
    }

    #[test]
    fn test_vg2svg_denied_http_access() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vg_output = output_path("access_seattle.vg.json");
        Command::cargo_bin("vl-convert")?
            .arg("vl2vg")
            .arg("-i").arg(vl_spec_path("seattle-weather"))
            .arg("-o").arg(&vg_output)
            .arg("--vl-version").arg("5.8")
            .assert()
            .success();

        let output = output_path("access_vg2svg.svg");
        Command::cargo_bin("vl-convert")?
            .arg("vg2svg")
            .arg("-i").arg(&vg_output)
            .arg("-o").arg(&output)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--allow-http-access").arg("false")
            .assert()
            .failure()
            .stderr(predicate::str::contains("access").or(predicate::str::contains("denied")));

        Ok(())
    }

    #[test]
    fn test_svg2png_denied_without_filesystem_root() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let temp = tempdir()?;
        let image_path = temp.path().join("inside.png");
        std::fs::write(&image_path, PNG_1X1)?;
        let svg_input = temp.path().join("input.svg");
        std::fs::write(
            &svg_input,
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><image href=\"{}\" width=\"1\" height=\"1\"/></svg>",
                file_href(&image_path)
            ),
        )?;

        let output = output_path("access_svg2png_denied.png");
        Command::cargo_bin("vl-convert")?
            .arg("svg2png")
            .arg("-i").arg(svg_input)
            .arg("-o").arg(&output)
            .arg("--allow-http-access").arg("false")
            .assert()
            .failure()
            .stderr(predicate::str::contains("Filesystem access denied").or(predicate::str::contains("access denied")));

        Ok(())
    }

    #[test]
    fn test_svg2png_allows_filesystem_root() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let temp = tempdir()?;
        let root = temp.path().join("root");
        std::fs::create_dir_all(&root)?;
        let image_path = root.join("inside.png");
        std::fs::write(&image_path, PNG_1X1)?;
        let svg_input = root.join("input.svg");
        std::fs::write(
            &svg_input,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><image href=\"inside.png\" width=\"1\" height=\"1\"/></svg>",
        )?;

        let output = output_path("access_svg2png_allowed.png");
        Command::cargo_bin("vl-convert")?
            .arg("svg2png")
            .arg("-i").arg(svg_input)
            .arg("-o").arg(&output)
            .arg("--allow-http-access").arg("false")
            .arg("--filesystem-root").arg(root)
            .assert()
            .success();
        assert!(!std::fs::read(&output)?.is_empty());

        Ok(())
    }

    #[test]
    fn test_svg2png_rejects_allowed_base_url_when_http_disabled() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let temp = tempdir()?;
        let svg_input = temp.path().join("input.svg");
        std::fs::write(
            &svg_input,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>",
        )?;

        let output = output_path("access_svg2png_invalid_config.png");
        Command::cargo_bin("vl-convert")?
            .arg("svg2png")
            .arg("-i").arg(svg_input)
            .arg("-o").arg(&output)
            .arg("--allow-http-access").arg("false")
            .arg("--allowed-base-url").arg("https://example.com")
            .assert()
            .failure()
            .stderr(predicate::str::contains("allowed_base_urls cannot be set when HTTP access is disabled"));

        Ok(())
    }

    #[test]
    fn test_vg2svg_allows_normalized_allowed_base_url() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let server = TestHttpServer::new(vec![(
            "/allowed/data.csv",
            TestHttpResponse::ok_csv("a,b\n1,2\n"),
        )]);
        let temp = tempdir()?;
        let vg_spec_path = temp.path().join("input.vg.json");
        let vg_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 20,
            "height": 20,
            "data": [{"name": "table", "url": server.url("/allowed/data.csv"), "format": {"type": "csv"}}],
            "scales": [
                {"name": "x", "type": "linear", "range": "width", "domain": {"data": "table", "field": "a"}},
                {"name": "y", "type": "linear", "range": "height", "domain": {"data": "table", "field": "b"}}
            ],
            "marks": [{
                "type": "symbol",
                "from": {"data": "table"},
                "encode": {"enter": {"x": {"scale": "x", "field": "a"}, "y": {"scale": "y", "field": "b"}}}
            }]
        });
        std::fs::write(&vg_spec_path, serde_json::to_string(&vg_spec)?)?;

        let output = output_path("access_vg2svg_allowed_base_url.svg");
        Command::cargo_bin("vl-convert")?
            .arg("vg2svg")
            .arg("-i").arg(&vg_spec_path)
            .arg("-o").arg(&output)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--allowed-base-url").arg(format!("{}/allowed", server.origin()))
            .assert()
            .success();

        assert!(!std::fs::read(&output)?.is_empty());
        Ok(())
    }

    #[test]
    fn test_vg2svg_denies_redirect_when_allowlist_configured() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let disallowed_server = TestHttpServer::new(vec![(
            "/data.csv",
            TestHttpResponse::ok_csv("a,b\n1,2\n"),
        )]);
        let allowed_server = TestHttpServer::new(vec![(
            "/redirect.csv",
            TestHttpResponse::redirect(&disallowed_server.url("/data.csv")),
        )]);

        let temp = tempdir()?;
        let vg_spec_path = temp.path().join("redirect.vg.json");
        let vg_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 20,
            "height": 20,
            "data": [{"name": "table", "url": allowed_server.url("/redirect.csv"), "format": {"type": "csv"}}],
            "scales": [
                {"name": "x", "type": "linear", "range": "width", "domain": {"data": "table", "field": "a"}},
                {"name": "y", "type": "linear", "range": "height", "domain": {"data": "table", "field": "b"}}
            ],
            "marks": [{
                "type": "symbol",
                "from": {"data": "table"},
                "encode": {"enter": {"x": {"scale": "x", "field": "a"}, "y": {"scale": "y", "field": "b"}}}
            }]
        });
        std::fs::write(&vg_spec_path, serde_json::to_string(&vg_spec)?)?;

        let output = output_path("access_vg2svg_redirect_denied.svg");
        Command::cargo_bin("vl-convert")?
            .arg("vg2svg")
            .arg("-i").arg(&vg_spec_path)
            .arg("-o").arg(&output)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--allowed-base-url").arg(allowed_server.origin())
            .assert()
            .failure()
            .stderr(predicate::str::contains("Redirected HTTP URLs are not allowed"));

        Ok(())
    }

    #[test]
    fn test_vg2svg_allows_redirect_without_allowlist() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let target_server = TestHttpServer::new(vec![(
            "/data.csv",
            TestHttpResponse::ok_csv("a,b\n1,2\n"),
        )]);
        let redirect_server = TestHttpServer::new(vec![(
            "/redirect.csv",
            TestHttpResponse::redirect(&target_server.url("/data.csv")),
        )]);

        let temp = tempdir()?;
        let vg_spec_path = temp.path().join("redirect_allowed.vg.json");
        let vg_spec = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega/v5.json",
            "width": 20,
            "height": 20,
            "data": [{"name": "table", "url": redirect_server.url("/redirect.csv"), "format": {"type": "csv"}}],
            "scales": [
                {"name": "x", "type": "linear", "range": "width", "domain": {"data": "table", "field": "a"}},
                {"name": "y", "type": "linear", "range": "height", "domain": {"data": "table", "field": "b"}}
            ],
            "marks": [{
                "type": "symbol",
                "from": {"data": "table"},
                "encode": {"enter": {"x": {"scale": "x", "field": "a"}, "y": {"scale": "y", "field": "b"}}}
            }]
        });
        std::fs::write(&vg_spec_path, serde_json::to_string(&vg_spec)?)?;

        let output = output_path("access_vg2svg_redirect_allowed.svg");
        Command::cargo_bin("vl-convert")?
            .arg("vg2svg")
            .arg("-i").arg(&vg_spec_path)
            .arg("-o").arg(&output)
            .arg("--font-dir").arg(test_font_dir())
            .assert()
            .success();

        assert!(!std::fs::read(&output)?.is_empty());
        Ok(())
    }
}

fn vl_spec_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join(format!("{}.vl.json", name));
    spec_path.to_str().unwrap().to_string()
}

fn format_locale_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("locale")
        .join("format")
        .join(format!("{}.json", name));
    spec_path.to_str().unwrap().to_string()
}

fn time_format_locale_path(name: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("locale")
        .join("time-format")
        .join(format!("{}.json", name));
    spec_path.to_str().unwrap().to_string()
}

fn load_expected_vg_spec(name: &str, vl_version: &str, pretty: bool) -> Option<String> {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(if pretty {
            format!("{}.vg.pretty.json", name)
        } else {
            format!("{}.vg.json", name)
        });

    if spec_path.exists() {
        let spec_str = fs::read_to_string(&spec_path)
            .unwrap_or_else(|_| panic!("Failed to read {:?}", spec_path));
        Some(spec_str)
    } else {
        None
    }
}

fn load_expected_svg(name: &str, vl_version: &str) -> String {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(format!("{}.svg", name));

    fs::read_to_string(spec_path).unwrap()
}

fn load_expected_png(name: &str, vl_version: &str, theme: Option<&str>) -> Option<DssimImage<f32>> {
    let vl_version = VlVersion::from_str(vl_version).unwrap();
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let image_path = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("vl-specs")
        .join("expected")
        .join(format!("{:?}", vl_version))
        .join(if let Some(theme) = theme {
            format!("{}-{}.png", name, theme)
        } else {
            format!("{}.png", name)
        });
    dssim::load_image(&Dssim::new(), image_path).ok()
}

fn output_path(filename: &str) -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    root_path
        .join("tests")
        .join("output")
        .join(filename)
        .to_str()
        .unwrap()
        .to_string()
}

fn test_font_dir() -> String {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fonts_dir = root_path
        .join("..")
        .join("vl-convert-rs")
        .join("tests")
        .join("fonts");
    fonts_dir.to_str().unwrap().to_string()
}

#[test]
fn check_no_command() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = Command::cargo_bin("vl-convert")?;

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Usage: vl-convert"));
    Ok(())
}

#[rustfmt::skip]
mod test_vl2vg {
    use std::fs;
    use std::process::Command;
    use crate::*;

    #[rstest]
    fn test(
        #[values(
            "v5_8",
            "v5_14",
            "v5_15",
            "v5_16",
            "v5_17",
            "v5_20",
            "v5_21",
            "v6_1",
            "v6_4",
        )]
        vl_version: &str,

        #[values("circle_binned", "seattle-weather", "stacked_bar_h")]
        name: &str,

        #[values(true, false)]
        pretty: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = if pretty {
            format!("{}_{}.pretty.vg.json", vl_version, name)
        } else {
            format!("{}_{}.vg.json", vl_version, name)
        };

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut cmd = cmd.arg("vl2vg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version);

        if pretty {
            cmd = cmd.arg("--pretty")
        }

        // Load expected
        match load_expected_vg_spec(name, vl_version, pretty) {
            Some(expected_str) => {
                cmd.assert().success();

                // Load written spec
                let output_str = fs::read_to_string(&output).unwrap();

                assert_eq!(expected_str, output_str)
            }
            None => {
                cmd.assert().failure();
            }
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2html_no_bundle {
    use std::fs;
    use std::process::Command;
    use crate::*;

    #[rstest]
    fn test(
        #[values(
            "5.8",
            "5.14",
            "5.15",
            "5.16",
            "5.17",
            "5.20",
            "5.21",
            "6.1",
            "6.4",
        )]
        vl_version: &str,

        #[values("circle_binned")]
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = format!("{}_{}.html", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2html")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version);
        cmd.assert().success();

        // Load written html
        let html_result = fs::read_to_string(&output).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(&format!("cdn.jsdelivr.net/npm/vega-lite@{vl_version}")));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega@6"));
        assert!(html_result.contains("cdn.jsdelivr.net/npm/vega-embed@6"));

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2html_bundle {
    use std::fs;
    use std::process::Command;
    use crate::*;

    #[rstest]
    fn test(
        #[values(
            "5.8",
            "5.14",
            "5.15",
            "5.16",
            "5.17",
            "5.20",
            "5.21",
            "6.1",
            "6.4",
        )]
        vl_version: &str,

        #[values("circle_binned")]
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = format!("{}_{}.html", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2html")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--bundle");
        cmd.assert().success();

        // Load written html
        let html_result = fs::read_to_string(&output).unwrap();

        // Check for expected patterns
        assert!(html_result.starts_with("<!DOCTYPE html>"));
        assert!(html_result.contains(vl_version));
        assert!(html_result.contains("<div id=\"vega-chart\">"));

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2svg {
    use std::fs;
    use std::process::Command;
    use crate::*;

    #[rstest]
    fn test(
        #[values(
            "v5_8",
        )]
        vl_version: &str,

        #[values("circle_binned", "stacked_bar_h")]
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let output_filename = format!("{}_{}.svg", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2svg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir());

        // Load expected
        let expected_str = load_expected_svg(name, vl_version);
        cmd.assert().success();

        // Load written spec
        let output_str = fs::read_to_string(&output).unwrap();
        assert_eq!(expected_str, output_str);

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png {
    use std::process::Command;
    use crate::*;

    #[rstest(name, scale,
        case("circle_binned", 1.0),
        case("stacked_bar_h", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}.png", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, None).unwrap();


        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png_theme_config {
    use std::process::Command;
    use crate::*;

    #[rstest(name, scale, theme,
    case("circle_binned", 1.0, "dark"),
    case("stacked_bar_h", 2.0, "vox")
    )]
    fn test(
        name: &str,
        scale: f32,
        theme: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}_theme.png", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        // Write config with background color to temporary file
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, r#"{{"background": "{}"}}"#, BACKGROUND_COLOR).unwrap();
        let config_path = config_file.path().to_str().unwrap();

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--theme").arg(theme)
            .arg("--config").arg(config_path)
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, Some(theme)).unwrap();

        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2png_locale {
    use std::process::Command;
    use crate::*;

    #[rstest(name, scale,
        case("stocks_locale", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}.png", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        // Test with locale path
        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path.clone())
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--format-locale").arg(format_locale_path("it-IT"))
            .arg("--time-format-locale").arg(time_format_locale_path("it-IT"))
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, None).unwrap();

        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        // Test with locale name
        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2png")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--format-locale").arg("it-IT")
            .arg("--time-format-locale").arg("it-IT")
            .arg("--scale").arg(scale.to_string());

        cmd.assert().success();

        // Load expected
        let expected_png = load_expected_png(name, vl_version, None).unwrap();

        // Load written spec
        let output_png = dssim::load_image(&Dssim::new(), &output).unwrap();

        let attr = Dssim::new();
        let (diff, _) = attr.compare(&expected_png, output_png);

        if diff > 0.0001 {
            panic!(
                "Images don't match for {}.png with diff {}",
                name, diff
            )
        }

        Ok(())
    }
}

#[rustfmt::skip]
mod test_vl2jpeg {
    use std::process::Command;
    use crate::*;

    #[rstest(name, scale,
    case("circle_binned", 1.0),
    case("stacked_bar_h", 2.0)
    )]
    fn test(
        name: &str,
        scale: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_version = "5_8";
        let output_filename = format!("{}_{}.jpeg", vl_version, name);

        let vl_path = vl_spec_path(name);
        let output = output_path(&output_filename);

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let cmd = cmd.arg("vl2jpeg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg(vl_version)
            .arg("--font-dir").arg(test_font_dir())
            .arg("--scale").arg(scale.to_string())
            .arg("--quality").arg("99");

        cmd.assert().success();

        // Load written spec
        let output_jpg = fs::read(&output).expect("Failed to read output image");
        assert_eq!(&output_jpg.as_slice()[..10], b"\xff\xd8\xff\xe0\x00\x10JFIF");

        Ok(())
    }
}

#[test]
fn test_ls_themes() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("vl-convert")?;
    let cmd = cmd.arg("ls-themes");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();
    assert!(output_str.contains("dark"));

    Ok(())
}

#[test]
fn test_cat_theme() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("vl-convert")?;
    let cmd = cmd.arg("cat-theme").arg("dark");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let output_str = String::from_utf8(output.stdout).unwrap();

    // Check for known background color entry
    assert!(output_str.contains(r##""background": "#333"##));

    Ok(())
}

// Helper functions for stdin/stdout tests
fn load_vl_spec_string(name: &str) -> String {
    let spec_path = vl_spec_path(name);
    fs::read_to_string(&spec_path).unwrap()
}

fn validate_png_header(data: &[u8]) -> bool {
    data.len() >= 8 && data[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
}

fn validate_jpeg_header(data: &[u8]) -> bool {
    data.len() >= 4 && data[0..4] == [0xFF, 0xD8, 0xFF, 0xE0]
}

fn validate_pdf_header(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == b"%PDF"
}

#[rustfmt::skip]
mod test_stdin_stdout {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use crate::*;

    // Test stdin text input with file output
    #[test]
    fn test_stdin_vl2vg_file_output() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("stdin_vl2vg.vg.json");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2vg")
            .arg("-o").arg(&output)
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output_result = child.wait_with_output()?;

        assert!(output_result.status.success());
        assert!(Path::new(&output).exists());

        let output_content = fs::read_to_string(&output)?;
        assert!(output_content.contains(r#""$schema""#));
        Ok(())
    }

    // Test stdout text output with file input
    #[test]
    fn test_file_input_vl2svg_stdout() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_path = vl_spec_path("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let output = cmd
            .arg("vl2svg")
            .arg("-i").arg(vl_path)
            .arg("--vl-version").arg("5.8")
            .output()?;

        assert!(output.status.success());
        let svg_str = String::from_utf8(output.stdout)?;
        assert!(svg_str.contains("<svg"));
        assert!(svg_str.contains("</svg>"));
        Ok(())
    }

    // Test full stdin+stdout pipeline for text formats
    #[test]
    fn test_stdin_stdout_vl2vg() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output = child.wait_with_output()?;

        assert!(output.status.success());
        let vg_str = String::from_utf8(output.stdout)?;
        assert!(vg_str.contains(r#""$schema""#));
        assert!(vg_str.contains("vega"));
        Ok(())
    }

    // Test stdin binary output to file
    #[test]
    fn test_stdin_vl2png_file_output() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("stdin_vl2png.png");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2png")
            .arg("-o").arg(&output)
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let result = child.wait()?;

        assert!(result.success());
        assert!(Path::new(&output).exists());

        let png_data = fs::read(&output)?;
        assert!(validate_png_header(&png_data));
        Ok(())
    }

    // Test explicit stdout override for binary (PNG)
    #[test]
    fn test_png_explicit_stdout_override() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2png")
            .arg("-o").arg("-")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output = child.wait_with_output()?;

        assert!(output.status.success());
        assert!(validate_png_header(&output.stdout));
        Ok(())
    }

    // Test explicit stdout override for binary (JPEG)
    #[test]
    fn test_jpeg_explicit_stdout_override() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2jpeg")
            .arg("-o").arg("-")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output = child.wait_with_output()?;

        assert!(output.status.success());
        assert!(validate_jpeg_header(&output.stdout));
        Ok(())
    }

    // Test explicit stdout override for binary (PDF)
    #[test]
    fn test_pdf_explicit_stdout_override() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2pdf")
            .arg("-o").arg("-")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output = child.wait_with_output()?;

        assert!(output.status.success());
        assert!(validate_pdf_header(&output.stdout));
        Ok(())
    }

    // Test error on invalid JSON via stdin
    #[test]
    fn test_stdin_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let invalid_json = "not valid json {";

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(invalid_json.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output = child.wait_with_output()?;

        assert!(!output.status.success());
        Ok(())
    }

    // Test implicit stdin (omit -i flag entirely)
    #[test]
    fn test_implicit_stdin_vl2vg() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("implicit_stdin.vg.json");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2vg")
            .arg("-o").arg(&output)
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let result = child.wait()?;

        assert!(result.success());
        assert!(Path::new(&output).exists());
        Ok(())
    }

    // Test explicit -i - for stdin
    #[test]
    fn test_explicit_stdin_dash() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("explicit_stdin_dash.vg.json");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2vg")
            .arg("-i").arg("-")
            .arg("-o").arg(&output)
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let result = child.wait()?;

        assert!(result.success());
        assert!(Path::new(&output).exists());
        Ok(())
    }

    // Test backward compatibility: existing file-to-file workflows unchanged
    #[test]
    fn test_backward_compat_file_to_file() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_path = vl_spec_path("circle_binned");
        let output = output_path("backward_compat.vg.json");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        cmd.arg("vl2vg")
            .arg("-i").arg(vl_path)
            .arg("-o").arg(&output)
            .arg("--vl-version").arg("5.8")
            .assert()
            .success();

        assert!(Path::new(&output).exists());
        let output_content = fs::read_to_string(&output)?;
        assert!(output_content.contains(r#""$schema""#));
        Ok(())
    }

    // Test vl2url with stdin support
    #[test]
    fn test_vl2url_stdin() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2url")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin); // Explicitly close stdin before wait
        let output = child.wait_with_output()?;

        assert!(output.status.success());
        let url = String::from_utf8(output.stdout)?;
        assert!(url.contains("https://vega.github.io/editor"));
        Ok(())
    }

    // Test vg2url with stdin support
    #[test]
    fn test_vg2url_stdin() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        // First convert VL to VG via stdin/stdout
        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1); // Explicitly close stdin before wait
        let vg_output = child1.wait_with_output()?;
        assert!(vg_output.status.success());

        // Now pipe VG to vg2url
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let mut child2 = cmd2
            .arg("vg2url")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        child2.stdin.as_mut().unwrap().write_all(&vg_output.stdout)?;
        let url_output = child2.wait_with_output()?;

        assert!(url_output.status.success());
        let url = String::from_utf8(url_output.stdout)?;
        assert!(url.contains("https://vega.github.io/editor"));
        Ok(())
    }

    // Test SVG subcommands with stdin/stdout
    #[test]
    fn test_svg2png_stdin_stdout() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        // First generate SVG via stdin/stdout
        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2svg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1); // Explicitly close stdin before wait
        let svg_output = child1.wait_with_output()?;
        assert!(svg_output.status.success());

        // Now convert SVG to PNG with explicit stdout override
        let output = output_path("svg2png_pipeline.png");
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let mut child2 = cmd2
            .arg("svg2png")
            .arg("-o").arg(&output)
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin2 = child2.stdin.take().unwrap();
        stdin2.write_all(&svg_output.stdout)?;
        drop(stdin2); // Explicitly close stdin before wait
        let result = child2.wait()?;

        assert!(result.success());
        let png_data = fs::read(&output)?;
        assert!(validate_png_header(&png_data));
        Ok(())
    }

    // Test HTML output to stdout
    #[test]
    fn test_vl2html_stdout() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_path = vl_spec_path("circle_binned");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let output = cmd
            .arg("vl2html")
            .arg("-i").arg(vl_path)
            .arg("--vl-version").arg("5.8")
            .output()?;

        assert!(output.status.success());
        let html_str = String::from_utf8(output.stdout)?;
        assert!(html_str.contains("<html"));
        assert!(html_str.contains("</html>"));
        Ok(())
    }

    // Test Vega subcommands with stdin
    #[test]
    fn test_vg2svg_stdin() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        // First convert VL to VG
        let vl_spec = load_vl_spec_string("circle_binned");

        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1); // Explicitly close stdin before wait
        let vg_output = child1.wait_with_output()?;
        assert!(vg_output.status.success());

        // Now convert VG to SVG via stdin/stdout
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let mut child2 = cmd2
            .arg("vg2svg")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin2 = child2.stdin.take().unwrap();
        stdin2.write_all(&vg_output.stdout)?;
        drop(stdin2); // Explicitly close stdin before wait
        let svg_output = child2.wait_with_output()?;

        assert!(svg_output.status.success());
        let svg_str = String::from_utf8(svg_output.stdout)?;
        assert!(svg_str.contains("<svg"));
        Ok(())
    }

    // Test empty stdin error
    #[test]
    fn test_empty_stdin_error() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write empty string to stdin
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"")?;
        drop(stdin);
        let output = child.wait_with_output()?;

        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("No input provided"));
        Ok(())
    }

    // Test vg2png with stdin
    #[test]
    fn test_vg2png_stdin() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("vg2png_stdin.png");

        // First convert VL to VG
        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1);
        let vg_output = child1.wait_with_output()?;
        assert!(vg_output.status.success());

        // Now convert VG to PNG via stdin
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let mut child2 = cmd2
            .arg("vg2png")
            .arg("-o").arg(&output)
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin2 = child2.stdin.take().unwrap();
        stdin2.write_all(&vg_output.stdout)?;
        drop(stdin2);
        let result = child2.wait()?;

        assert!(result.success());
        let png_data = fs::read(&output)?;
        assert!(validate_png_header(&png_data));
        Ok(())
    }

    // Test vg2jpeg with stdin
    #[test]
    fn test_vg2jpeg_stdin() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("vg2jpeg_stdin.jpg");

        // First convert VL to VG
        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1);
        let vg_output = child1.wait_with_output()?;
        assert!(vg_output.status.success());

        // Now convert VG to JPEG via stdin
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let mut child2 = cmd2
            .arg("vg2jpeg")
            .arg("-o").arg(&output)
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin2 = child2.stdin.take().unwrap();
        stdin2.write_all(&vg_output.stdout)?;
        drop(stdin2);
        let result = child2.wait()?;

        assert!(result.success());
        let jpeg_data = fs::read(&output)?;
        assert!(validate_jpeg_header(&jpeg_data));
        Ok(())
    }

    // Test vg2pdf with stdin
    #[test]
    fn test_vg2pdf_stdin() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("vg2pdf_stdin.pdf");

        // First convert VL to VG
        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1);
        let vg_output = child1.wait_with_output()?;
        assert!(vg_output.status.success());

        // Now convert VG to PDF via stdin
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let mut child2 = cmd2
            .arg("vg2pdf")
            .arg("-o").arg(&output)
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin2 = child2.stdin.take().unwrap();
        stdin2.write_all(&vg_output.stdout)?;
        drop(stdin2);
        let result = child2.wait()?;

        assert!(result.success());
        let pdf_data = fs::read(&output)?;
        assert!(validate_pdf_header(&pdf_data));
        Ok(())
    }

    #[test]
    fn test_vl2url_file_output() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        let vl_spec = load_vl_spec_string("circle_binned");
        let output = output_path("vl2url_output.txt");

        let mut cmd = Command::cargo_bin("vl-convert")?;
        let mut child = cmd
            .arg("vl2url")
            .arg("-o").arg(&output)
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(vl_spec.as_bytes())?;
        drop(stdin);
        let result = child.wait()?;

        assert!(result.success());
        let url = fs::read_to_string(&output)?;
        assert!(url.contains("https://vega.github.io/editor"));
        Ok(())
    }

    #[test]
    fn test_vg2url_file_output() -> Result<(), Box<dyn std::error::Error>> {
        initialize();

        // First convert VL to VG
        let vl_spec = load_vl_spec_string("circle_binned");
        let vg_output_path = output_path("vg_spec_for_url.vg.json");

        let mut cmd1 = Command::cargo_bin("vl-convert")?;
        let mut child1 = cmd1
            .arg("vl2vg")
            .arg("--vl-version").arg("5.8")
            .arg("-o").arg(&vg_output_path)
            .stdin(Stdio::piped())
            .spawn()?;

        let mut stdin1 = child1.stdin.take().unwrap();
        stdin1.write_all(vl_spec.as_bytes())?;
        drop(stdin1);
        let result1 = child1.wait()?;
        assert!(result1.success());

        // Now convert VG to URL with file output
        let url_output = output_path("vg2url_output.txt");
        let mut cmd2 = Command::cargo_bin("vl-convert")?;
        let result2 = cmd2
            .arg("vg2url")
            .arg("-i").arg(&vg_output_path)
            .arg("-o").arg(&url_output)
            .status()?;

        assert!(result2.success());
        let url = fs::read_to_string(&url_output)?;
        assert!(url.contains("https://vega.github.io/editor"));
        Ok(())
    }
}
