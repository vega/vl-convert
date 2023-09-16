use std::fs;
use usvg::fontdb::Database;
use usvg::TreeParsing;
use vl_convert_pdf::svg_to_pdf;

fn main() {
    let tree = usvg::Tree::from_str(r#"
<svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <text id="text1" x="100" y="100" text-anchor="middle" font-family="Arial" font-size="20" fill="black">
        Hello, World!
    </text>
    <rect id="frame" x="1" y="1" width="198" height="198" fill="none" stroke="black"/>
</svg>
    "#, &Default::default()).unwrap();

    let mut font_db = Database::new();
    font_db.load_system_fonts();

    let pdf_bytes = svg_to_pdf(&tree, &font_db, 1.0).unwrap();
    fs::write("target/hello.pdf", pdf_bytes).unwrap();
}
