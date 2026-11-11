use vl_convert_rs::{serde_json::json, PgfplotsOpts, ValueOrString, VlConverter};

fn line() -> serde_json::Value {
    json!({
        "data": {"values": [{"x": 1, "y": 2}, {"x": 2, "y": 4}]},
        "mark": "line",
        "encoding": {
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"}
        }
    })
}

#[tokio::test]
async fn native_data_axes_and_text_not_outlines() {
    let converter = VlConverter::new();
    let first = converter
        .vegalite_to_pgfplots(line(), PgfplotsOpts::default())
        .await
        .unwrap();
    let second = converter
        .vegalite_to_pgfplots(line(), PgfplotsOpts::default())
        .await
        .unwrap();
    assert_eq!(first.tex, second.tex);
    for expected in ["\\begin{axis}", "\\addplot", "(1,2)", "(2,4)", "xlabel={x}"] {
        assert!(first.tex.contains(expected), "missing {expected}");
    }
    for forbidden in ["\\pgfpath", "includegraphics", "hide axis", "documentclass"] {
        assert!(!first.tex.contains(forbidden), "unexpected {forbidden}");
    }
}

#[tokio::test]
async fn json_string_and_value_agree() {
    let converter = VlConverter::new();
    let value = line();
    let a = converter
        .vegalite_to_pgfplots(value.clone(), PgfplotsOpts::default())
        .await
        .unwrap();
    let b = converter
        .vegalite_to_pgfplots(value.to_string(), PgfplotsOpts::default())
        .await
        .unwrap();
    assert_eq!(a.tex, b.tex);
}

#[tokio::test]
async fn unsupported_inputs_fail_without_poisoning_worker() {
    let converter = VlConverter::new();
    for input in [
        json!({"marks": []}),
        json!({"mark": "line", "data": {"url": "file:///etc/passwd"}}),
        json!({"mark": "line", "params": [{"name": "brush", "select": "interval"}]}),
        json!({"mark": "line", "transform": [{"calculate": "1+1", "as": "y"}]}),
    ] {
        assert!(converter
            .vegalite_to_pgfplots(input, PgfplotsOpts::default())
            .await
            .is_err());
    }
    assert!(converter
        .vegalite_to_pgfplots("not JSON".to_owned(), PgfplotsOpts::default())
        .await
        .is_err());
    assert!(converter
        .vegalite_to_pgfplots(line(), PgfplotsOpts::default())
        .await
        .is_ok());
}

#[tokio::test]
async fn raw_math_requires_explicit_opt_in() {
    let mut spec = line();
    spec["encoding"]["x"]["title"] = json!(r"$t_{\mathrm{exp}}$");
    let converter = VlConverter::new();
    let escaped = converter
        .vegalite_to_pgfplots(spec.clone(), PgfplotsOpts::default())
        .await
        .unwrap();
    assert!(escaped.tex.contains(r"\$t\_\{\textbackslash{}mathrm\{exp\}\}\$"));
    let trusted = converter
        .vegalite_to_pgfplots(
            spec,
            PgfplotsOpts {
                standalone: true,
                tex_labels: true,
            },
        )
        .await
        .unwrap();
    assert!(trusted.tex.contains(r"xlabel={$t_{\mathrm{exp}}$}"));
    assert!(trusted.tex.starts_with("\\documentclass"));
    assert!(trusted.tex.ends_with("\\end{document}\n"));
}

#[tokio::test]
async fn quoted_input_remains_data_not_javascript() {
    let converter = VlConverter::new();
    let mut spec = line();
    spec["encoding"]["x"]["title"] =
        json!("');globalThis.compromised=1;//\n\u{2028}\\input{secret}");
    let result = converter
        .vegalite_to_pgfplots(spec, PgfplotsOpts::default())
        .await
        .unwrap();
    assert!(result.tex.contains("textbackslash{}input"));
    assert!(converter
        .vegalite_to_pgfplots(line(), PgfplotsOpts::default())
        .await
        .is_ok());
}

#[tokio::test]
async fn oversized_input_is_rejected_before_v8() {
    let converter = VlConverter::new();
    let source = ValueOrString::JsonString(" ".repeat(8 * 1024 * 1024 + 1));
    let err = converter
        .vegalite_to_pgfplots(source, PgfplotsOpts::default())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("8 MiB"));
}

#[tokio::test]
async fn explicit_uncertainty_is_emitted_as_native_error_bars() {
    let mut spec = line();
    spec["mark"] = json!("errorbar");
    spec["data"]["values"] = json!([{"x": 1, "y": 4, "e": 0.25}]);
    spec["encoding"]["yError"] = json!({"field": "e"});
    let tex = VlConverter::new()
        .vegalite_to_pgfplots(spec, PgfplotsOpts::default())
        .await
        .unwrap()
        .tex;
    assert!(tex.contains("error bars/.cd"));
    assert!(tex.contains("0.25"));
    assert!(!tex.contains("pgfpath"));
}
