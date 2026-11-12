use assert_cmd::Command;
use predicates::str::contains;

const SPEC: &str = r#"{"data":{"values":[{"x":1,"y":2}]},"mark":"point","encoding":{"x":{"field":"x","type":"quantitative"},"y":{"field":"y","type":"quantitative"}}}"#;

#[test]
fn inline_json_on_stdin_native_tex_on_stdout() {
    Command::new(env!("CARGO_BIN_EXE_vl2pgfplots"))
        .arg("--standalone")
        .write_stdin(SPEC)
        .assert()
        .success()
        .stdout(contains("\\begin{axis}"))
        .stdout(contains("(1,2)"));
}

#[test]
fn bad_input_leaves_existing_output_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("figure.tex");
    std::fs::write(&output, "existing scientific figure").unwrap();
    Command::new(env!("CARGO_BIN_EXE_vl2pgfplots"))
        .arg("--output")
        .arg(&output)
        .write_stdin(r#"{"marks":[]}"#)
        .assert()
        .failure()
        .stderr(contains("unsupported"));
    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        "existing scientific figure"
    );
}

#[test]
fn input_and_output_files() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("figure.json");
    let output = dir.path().join("figure.tex");
    std::fs::write(&input, SPEC).unwrap();
    Command::new(env!("CARGO_BIN_EXE_vl2pgfplots"))
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();
    assert!(std::fs::read_to_string(output).unwrap().contains("(1,2)"));
}
