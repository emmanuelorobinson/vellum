use vellum_types::{Document, FORMAT_VERSION, brand};

#[test]
fn format_version_is_one() {
    assert_eq!(FORMAT_VERSION, 1);
}

#[test]
fn brand_constants_are_the_canonical_ones() {
    assert_eq!(brand::NAME, "Vellum");
    assert_eq!(brand::TAGLINE, "Design where your code lives.");
}

#[test]
fn a_new_document_is_at_the_current_format_version() {
    let json = serde_json::to_value(Document::new("Launch")).expect("serialise");
    assert_eq!(json["meta"]["formatVersion"], FORMAT_VERSION);
}

#[test]
fn the_document_has_a_json_schema() {
    let schema = schemars::schema_for!(Document);
    let text = serde_json::to_string(&schema).expect("serialise schema");
    assert!(text.contains("formatVersion"), "schema was: {text}");
}
