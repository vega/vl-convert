#[test]
fn test_fonts() {
    let mut fontdb = fontdb::Database::default();
    // fontdb.load_system_fonts();
    fontdb.load_fonts_dir("/Users/jonmmease/VegaFusion/repos/vl-convert/scratch/bugs/matter");
    for face in fontdb.faces() {
        println!("{:#?}", face);
    }
}
