use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn svg_on_stdin_tex_on_stdout() {
    Command::new(env!("CARGO_BIN_EXE_vl2pgfplots"))
        .args(["--input-format", "svg", "--standalone"])
        .write_stdin(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="10" height="10"/></svg>"#)
        .assert().success().stdout(contains("\\begin{axis}"));
}

#[test]
fn bad_input_leaves_existing_output_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("figure.tex");
    std::fs::write(&output, "existing scientific figure").unwrap();
    Command::new(env!("CARGO_BIN_EXE_vl2pgfplots"))
        .args(["--input-format", "svg", "--output"])
        .arg(&output).write_stdin("not SVG")
        .assert().failure().stderr(contains("invalid SVG"));
    assert_eq!(std::fs::read_to_string(output).unwrap(), "existing scientific figure");
}

#[test]
fn inappropriate_version_flag_fails() {
    Command::new(env!("CARGO_BIN_EXE_vl2pgfplots"))
        .args(["--input-format", "svg", "--vl-version", "v6_4"])
        .write_stdin("").assert().failure()
        .stderr(contains("only applies to Vega-Lite"));
}
