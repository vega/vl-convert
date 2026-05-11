mod common;

use common::*;

#[test]
fn javascript_bundle_outputs_default_bundle() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut cmd = vl_convert_cmd()?;
    let output = cmd
        .arg("--vlc-config")
        .arg("disabled")
        .arg("javascript-bundle")
        .output()?;

    assert!(
        output.status.success(),
        "javascript-bundle failed with status {:?}; stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let bundle = String::from_utf8(output.stdout)?;
    assert!(bundle.contains("vegaEmbed"));
    Ok(())
}

#[test]
fn javascript_bundle_wraps_snippet() -> Result<(), Box<dyn std::error::Error>> {
    initialize();

    let mut snippet = NamedTempFile::new()?;
    snippet.write_all(b"console.log('vlc javascript bundling marker');")?;

    let mut cmd = vl_convert_cmd()?;
    let output = cmd
        .arg("--vlc-config")
        .arg("disabled")
        .arg("javascript-bundle")
        .arg("--snippet")
        .arg(snippet.path())
        .output()?;

    assert!(
        output.status.success(),
        "javascript-bundle --snippet failed with status {:?}; stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let bundle = String::from_utf8(output.stdout)?;
    assert!(bundle.contains("vlc javascript bundling marker"));
    Ok(())
}
