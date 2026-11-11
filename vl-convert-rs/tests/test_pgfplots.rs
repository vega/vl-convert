use vl_convert_rs::{svg_to_pgfplots, PgfplotsOpts, VlConverter, VlOpts};

fn svg(body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="90">{body}</svg>"#)
}

#[test]
fn deterministic_self_contained_output() {
    let source = svg(r##"<path d="M10 20L50 20L30 40Z" fill="#4080bf"/>"##);
    let opts = PgfplotsOpts { standalone: true };
    let a = svg_to_pgfplots(&source, opts).unwrap();
    assert_eq!(a, svg_to_pgfplots(&source, opts).unwrap());
    assert!(a.starts_with("\\documentclass"));
    assert!(a.ends_with("\\end{document}\n"));
    assert!(a.contains("{RGB}{64,128,191}"));
    assert!(a.contains("{7.500000bp}{-15.000000bp}"));
    assert!(!a.contains("includegraphics"));
}

#[test]
fn fragment_has_no_document_wrapper() {
    let a = svg_to_pgfplots(&svg(""), PgfplotsOpts::default()).unwrap();
    assert!(!a.contains("documentclass"));
    assert!(a.contains("width=90.000000bp,height=67.500000bp"));
}

#[test]
fn curves_strokes_and_transform_are_lowered() {
    let source = svg(r#"<g transform="translate(10 5) scale(2 1)">
      <path d="M0 0Q10 20 20 0C30 20 40 20 50 0" fill="none"
            stroke="black" stroke-width="2" stroke-dasharray="3 2"
            stroke-linecap="round" stroke-linejoin="round"/></g>"#);
    let result = svg_to_pgfplots(&source, PgfplotsOpts::default()).unwrap();
    assert!(result.contains("\\pgfpathcurveto") || result.contains("\\pgfpathquadraticcurveto"));
    // Strokes are outlines, not backend-dependent PGF pens.
    assert!(result.contains("\\pgfusepath{fill}"));
    assert!(!result.contains("\\pgfusepath{stroke}"));
}

#[test]
fn clips_opacity_and_evenodd_are_retained() {
    let source = svg(r##"<defs><clipPath id="c"><rect x="10" y="10" width="30" height="30"/></clipPath></defs>
      <g clip-path="url(#c)" opacity="0.5"><path fill-rule="evenodd"
       d="M0 0H60V60H0Z M10 10H50V50H10Z"/></g>"##);
    let tex = svg_to_pgfplots(&source, PgfplotsOpts::default()).unwrap();
    assert!(tex.contains("\\pgfseteorule"));
    assert!(tex.matches("\\pgfusepath{clip}").count() >= 2);
    assert!(tex.contains("opacity=0.500000,transparency group"));
}

#[test]
fn untrusted_text_is_not_tex() {
    let source = svg(r#"<text x="5" y="20">\input{secret} % &amp; $x$</text>"#);
    let tex = svg_to_pgfplots(&source, PgfplotsOpts::default()).unwrap();
    assert!(!tex.contains("\\input"));
    assert!(!tex.contains("secret"));
    assert!(!tex.contains("$x$"));
    assert!(tex.contains("\\pgfusepath{fill}"));
}

#[test]
fn unsupported_features_fail_closed() {
    for body in [
        r#"<image href="file:///etc/passwd"/>"#,
        r#"<foreignObject/>"#,
        r#"<rect width="10" height="10" fill="url(https://example.invalid/g.svg#gradient)"/>"#,
        r#"<script/>"#,
        r#"<style>rect { fill: red }</style>"#,
        r#"<linearGradient id="g"/>"#,
        r#"<mask id="m"/>"#,
        r#"<path d="M0 0L10 10" vector-effect="non-scaling-stroke"/>"#,
        r#"<use href="https://example.invalid/chart.svg#x"/>"#,
        r#"<path d="M0 0L10 10" style="filter:blur(2px)"/>"#,
    ] {
        assert!(svg_to_pgfplots(&svg(body), PgfplotsOpts::default()).is_err(), "{body}");
    }
    for source in [
        r#"<svg width="1" height="1"/>"#,
        r#"<!DOCTYPE svg [<!ENTITY x "x">]><svg xmlns="http://www.w3.org/2000/svg">&x;</svg>"#,
        "not XML",
    ] {
        assert!(svg_to_pgfplots(source, PgfplotsOpts::default()).is_err());
    }
}

#[test]
fn complex_clip_unions_are_not_approximated() {
    let source = svg(r##"<defs><clipPath id="c"><circle r="20"/><circle cx="20" r="20"/></clipPath></defs><g clip-path="url(#c)"><rect width="60" height="60"/></g>"##);
    assert!(svg_to_pgfplots(&source, PgfplotsOpts::default()).is_err());
}

#[tokio::test]
async fn chart_types_reuse_the_official_compiler() {
    let converter = VlConverter::new();
    for mark in ["point", "line", "bar", "area", "boxplot", "errorbar", "errorband"] {
        let spec = serde_json::json!({
            "width": 180, "height": 120,
            "data": {"values": [
                {"x": 1, "y": 2}, {"x": 1, "y": 3},
                {"x": 2, "y": 4}, {"x": 2, "y": 6}
            ]},
            "mark": mark,
            "encoding": {
                "x": {"field": "x", "type": "ordinal"},
                "y": {"field": "y", "type": "quantitative"}
            }
        });
        let out = converter.vegalite_to_pgfplots(spec, VlOpts::default(), PgfplotsOpts::default())
            .await.unwrap_or_else(|err| panic!("{mark}: {err}"));
        assert!(out.tex.contains("\\begin{axis}"));
        assert!(out.tex.contains("\\pgfusepath{fill}"));
    }
}

#[test]
fn dashed_strokes_are_not_silently_solid() {
    let plain = svg(r#"<path d="M10 40H110" fill="none" stroke="black" stroke-width="2"/>"#);
    let dashed = plain.replace("stroke-width=\"2\"", "stroke-width=\"2\" stroke-dasharray=\"5 5\"");
    let solid_tex = svg_to_pgfplots(&plain, PgfplotsOpts::default()).unwrap();
    let dashed_tex = svg_to_pgfplots(&dashed, PgfplotsOpts::default()).unwrap();
    assert_ne!(solid_tex, dashed_tex);
    assert!(dashed_tex.matches("\\pgfpathmoveto").count() > solid_tex.matches("\\pgfpathmoveto").count());
}
