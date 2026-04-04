#![allow(deprecated)]

mod common;

use common::*;
use std::process::Stdio;

fn log_scale_spec() -> String {
    r#"{
        "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
        "data": {"values": [{"a": "A", "v": 100}, {"a": "B", "v": 200}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "v", "type": "quantitative", "scale": {"type": "log"}}
        }
    }"#
    .to_string()
}

#[test]
fn test_log_level_warn_shows_warnings() -> Result<(), Box<dyn std::error::Error>> {
    initialize();
    let spec = log_scale_spec();

    let mut cmd = Command::cargo_bin("vl-convert")?;
    let mut child = cmd
        .arg("--log-level")
        .arg("warn")
        .arg("vl2svg")
        .arg("-o")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(spec.as_bytes())?;
    drop(stdin);
    let output = child.wait_with_output()?;

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Log scale"),
        "Expected 'Log scale' warning in stderr, got: {stderr}"
    );
    Ok(())
}

#[test]
fn test_log_level_error_hides_warnings() -> Result<(), Box<dyn std::error::Error>> {
    initialize();
    let spec = log_scale_spec();

    let mut cmd = Command::cargo_bin("vl-convert")?;
    let mut child = cmd
        .arg("--log-level")
        .arg("error")
        .arg("vl2svg")
        .arg("-o")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(spec.as_bytes())?;
    drop(stdin);
    let output = child.wait_with_output()?;

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Log scale"),
        "Expected no 'Log scale' warning at error level, got: {stderr}"
    );
    Ok(())
}
