#![allow(deprecated)]

mod common;

use common::*;
use std::process::Stdio;

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
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

#[test]
fn test_vg2svg_with_inline_plugin() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    // Inline plugin source passed directly as the --vega-plugin value.
    // The spec uses a custom color scheme registered by the plugin.
    let plugin_source =
        "export default function(vega) { vega.scheme('clischeme', ['red', 'green', 'blue']); }";

    let spec_path = vg_spec_path("plugin_custom_scheme");
    // plugin_custom_scheme.vg.json uses "testscheme" -- write a variant that uses "clischeme"
    let spec_str = fs::read_to_string(&spec_path)?;
    let spec_str = spec_str.replace("\"testscheme\"", "\"clischeme\"");
    let mut spec_file = NamedTempFile::new()?;
    spec_file.write_all(spec_str.as_bytes())?;

    let mut cmd = Command::cargo_bin("vl-convert")?;
    let output = cmd
        .arg("vg2svg")
        .arg("-i")
        .arg(spec_file.path())
        .arg("--vega-plugin")
        .arg(plugin_source)
        .output()?;

    assert!(
        output.status.success(),
        "vg2svg with --vega-plugin failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout)?;
    assert!(svg.contains("<svg"), "output should be valid SVG");
    // The plugin registered 'clischeme' with ['red', 'green', 'blue'].
    // Vega renders fill="red" for the first bar.
    assert!(
        svg.contains(r#"fill="red""#),
        "SVG should contain fill=\"red\" from the plugin-registered scheme"
    );
    Ok(())
}

#[test]
fn test_vg2svg_with_file_plugin() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    // Plugin written to a .js file -- exercises the file-path resolution
    // in normalize_converter_config().
    let mut plugin_file = NamedTempFile::with_suffix(".js")?;
    plugin_file.write_all(
        b"export default function(vega) { vega.expressionFunction('cliDouble', (x) => x * 2); }",
    )?;

    // Minimal Vega spec that renders the expression function result as text.
    let spec = serde_json::json!({
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 100, "height": 100,
        "marks": [{"type": "text", "encode": {"enter": {
            "text": {"signal": "cliDouble(21)"},
            "x": {"value": 50}, "y": {"value": 50}
        }}}]
    });
    let mut spec_file = NamedTempFile::with_suffix(".json")?;
    spec_file.write_all(serde_json::to_string(&spec)?.as_bytes())?;

    let mut cmd = Command::cargo_bin("vl-convert")?;
    let output = cmd
        .arg("vg2svg")
        .arg("-i")
        .arg(spec_file.path())
        .arg("--vega-plugin")
        .arg(plugin_file.path())
        .output()?;

    assert!(
        output.status.success(),
        "vg2svg with file --vega-plugin failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = String::from_utf8(output.stdout)?;
    assert!(svg.contains("<svg"), "output should be valid SVG");
    assert!(svg.contains("42"), "SVG should contain cliDouble(21) = 42");
    Ok(())
}
