mod common;

use common::*;
use std::process::{Command, Output, Stdio};

const SIMPLE_VL_SPEC: &str = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "data": {"values": [{"category": "A", "value": 1}]},
  "mark": "bar",
  "encoding": {
    "x": {"field": "category", "type": "nominal"},
    "y": {"field": "value", "type": "quantitative"}
  }
}"#;

fn run_with_stdin(
    cmd: &mut Command,
    stdin_text: &str,
) -> Result<Output, Box<dyn std::error::Error>> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("stdin must be piped");
    stdin.write_all(stdin_text.as_bytes())?;
    drop(stdin);
    Ok(child.wait_with_output()?)
}

#[test]
fn vl2sg_outputs_json() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = vl_convert_cmd()?;
    cmd.arg("--vlc-config").arg("disabled").arg("vl2sg");
    let output = run_with_stdin(&mut cmd, SIMPLE_VL_SPEC)?;

    assert!(
        output.status.success(),
        "vl2sg failed with status {:?}; stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let scenegraph: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(scenegraph.is_object(), "expected scenegraph object");
    Ok(())
}

#[test]
fn vg2sg_outputs_msgpack() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = vl_convert_cmd()?;
    let output = cmd
        .arg("--vlc-config")
        .arg("disabled")
        .arg("vg2sg")
        .arg("--format")
        .arg("msgpack")
        .arg("-i")
        .arg(vg_spec_path("label_scatter_plot"))
        .output()?;

    assert!(
        output.status.success(),
        "vg2sg --format msgpack failed with status {:?}; stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let scenegraph: serde_json::Value = rmp_serde::from_slice(&output.stdout)?;
    assert!(scenegraph.is_object(), "expected scenegraph object");
    Ok(())
}

#[test]
fn scenegraph_pretty_rejects_msgpack() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = vl_convert_cmd()?;
    cmd.arg("--vlc-config")
        .arg("disabled")
        .arg("vl2sg")
        .arg("--format")
        .arg("msgpack")
        .arg("--pretty");
    let output = run_with_stdin(&mut cmd, SIMPLE_VL_SPEC)?;
    assert!(!output.status.success(), "expected command to fail");
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("--pretty is only valid with --format json"));
    Ok(())
}
