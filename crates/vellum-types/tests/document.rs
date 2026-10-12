//! The document (ARCHITECTURE §4.1): the meta block and the sections a file holds. Every example is in
//! canonical form (a field at its default is left out), so it must print back exactly as it was read.

use serde_json::{Value as Json, json};
use vellum_types::{Document, DocumentKind, PageKind};

fn canonical(input: Json) -> Document {
    let doc: Document =
        serde_json::from_value(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"));
    let back = serde_json::to_value(&doc).expect("serialise");
    assert_eq!(
        back, input,
        "the document did not print back as it was read"
    );
    doc
}

fn rejected(input: Json) -> String {
    serde_json::from_value::<Document>(input.clone())
        .err()
        .unwrap_or_else(|| panic!("{input} should have been rejected"))
        .to_string()
}

fn meta() -> Json {
    json!({"formatVersion": 1, "name": "Launch", "createdWith": "Vellum 0.1.0"})
}

#[test]
fn a_new_document_is_its_meta_block_and_no_pages() {
    let json = serde_json::to_value(Document::new("Launch")).expect("serialise");
    assert_eq!(json["meta"]["formatVersion"], 1);
    assert_eq!(json["meta"]["name"], "Launch");
    assert!(
        json["meta"]["createdWith"]
            .as_str()
            .is_some_and(|s| s.starts_with("Vellum ")),
        "createdWith names the app: {json}"
    );
    assert_eq!(json["pages"], json!([]));
    assert!(
        json.get("variables").is_none(),
        "an empty section is left out: {json}"
    );
}

#[test]
fn a_minimal_document_prints_back_as_it_was_read() {
    canonical(json!({"meta": meta(), "pages": []}));
}

#[test]
fn every_section_of_a_document_round_trips() {
    canonical(json!({
        "meta": meta(),
        "imports": {"system": "design/system.lib.vellum"},
        "themes": {"Mode": ["light", "dark"], "Device": ["phone", "tablet"]},
        "variables": {
            "va000001": {"type": "number", "name": "space/md", "values": [{"value": 12.0}]}
        },
        "styles": {"st000001": {"type": "effect", "name": "Card shadow", "effects": []}},
        "fonts": {"fnt000001": {"family": "Geist"}},
        "components": {
            "comp000001": {
                "name": "Button",
                "root": {
                    "type": "frame",
                    "id": "btn000001",
                    "name": "Button",
                    "width": "hug",
                    "height": "hug"
                }
            }
        },
        "pages": [{"id": "page000001", "name": "Canvas"}]
    }));
}

#[test]
fn a_library_is_a_document_of_kind_library() {
    let doc = canonical(json!({
        "meta": {"formatVersion": 1, "name": "Starter", "createdWith": "Vellum 0.1.0", "kind": "library"},
        "pages": []
    }));
    assert_eq!(doc.meta.kind, DocumentKind::Library);
}

#[test]
fn the_document_kind_is_one_of_two_words_and_documents_leave_it_out() {
    let doc = canonical(json!({"meta": meta(), "pages": []}));
    assert_eq!(doc.meta.kind, DocumentKind::Document);
    let message = rejected(json!({
        "meta": {"formatVersion": 1, "name": "x", "createdWith": "y", "kind": "deck"},
        "pages": []
    }));
    assert!(message.contains("deck"), "{message}");
}

#[test]
fn pages_keep_their_order_and_a_slides_page_says_so() {
    let doc = canonical(json!({
        "meta": meta(),
        "pages": [
            {"id": "page000002", "name": "Overview"},
            {"id": "page000001", "name": "Deck", "kind": "slides"}
        ]
    }));
    let names: Vec<&str> = doc.pages.iter().map(|page| page.name.as_str()).collect();
    assert_eq!(names, ["Overview", "Deck"]);
    assert_eq!(doc.pages[0].kind, PageKind::Canvas);
    assert_eq!(doc.pages[1].kind, PageKind::Slides);
}

#[test]
fn a_page_holds_its_nodes_in_reading_order() {
    canonical(json!({
        "meta": meta(),
        "pages": [{
            "id": "page000001",
            "name": "Canvas",
            "children": [
                {"type": "rect", "id": "rect000001", "name": "Back", "width": 320.0, "height": 200.0},
                {"type": "rect", "id": "rect000002", "name": "Front", "width": 80.0, "height": 40.0}
            ]
        }]
    }));
}

#[test]
fn a_page_id_is_an_id_and_cannot_hold_a_slash() {
    let message = rejected(json!({"meta": meta(), "pages": [{"id": "a/b", "name": "Canvas"}]}));
    assert!(message.contains('/'), "{message}");
}

#[test]
fn a_document_refuses_unknown_fields_and_an_incomplete_meta_block() {
    assert!(rejected(json!({"meta": meta(), "pages": [], "layers": []})).contains("layers"));
    assert!(rejected(json!({"pages": []})).contains("meta"));
    let message = rejected(json!({"meta": {"formatVersion": 1, "name": "x"}, "pages": []}));
    assert!(message.contains("createdWith"), "{message}");
}

#[test]
fn imports_map_an_alias_to_a_relative_library_path() {
    let doc = canonical(json!({
        "meta": meta(),
        "imports": {"system": "design/system.lib.vellum"},
        "pages": []
    }));
    assert_eq!(doc.imports["system"], "design/system.lib.vellum");
}

#[test]
fn themes_keep_each_axis_in_order_so_the_first_value_is_the_default() {
    let doc = canonical(json!({
        "meta": meta(),
        "themes": {"Mode": ["light", "dark"]},
        "pages": []
    }));
    assert_eq!(doc.themes["Mode"], ["light", "dark"]);
}

#[test]
fn the_schema_names_every_section_of_a_document() {
    let text = serde_json::to_value(schemars::schema_for!(Document))
        .expect("serialise")
        .to_string();
    for name in [
        "formatVersion",
        "createdWith",
        "imports",
        "themes",
        "variables",
        "styles",
        "fonts",
        "components",
        "pages",
        "library",
        "slides",
    ] {
        assert!(text.contains(name), "the schema has no {name}");
    }
}
