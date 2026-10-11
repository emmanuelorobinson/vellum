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
fn placeholder_document_serialises_as_camel_case_json() {
    let json = serde_json::to_string(&Document::new()).expect("serialise");
    assert_eq!(json, r#"{"meta":{"formatVersion":1}}"#);
}

#[test]
fn placeholder_document_has_a_json_schema() {
    let schema = schemars::schema_for!(Document);
    let text = serde_json::to_string(&schema).expect("serialise schema");
    assert!(text.contains("formatVersion"), "schema was: {text}");
}
