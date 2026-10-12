//! Applying a props patch to a node, and finding the variables a value refers to. The op layer (vellum-core)
//! needs both and has no serde, so they live with the types.

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};
use vellum_types::{Node, NodePatch, VariableId, referenced_variables};

fn node(value: Json) -> Node {
    serde_json::from_value(value).expect("a valid node")
}

fn patch(value: Json) -> NodePatch {
    let Json::Object(map) = value else {
        panic!("a patch is an object")
    };
    NodePatch::new(map.into_iter().collect::<BTreeMap<_, _>>()).expect("known properties")
}

fn frame() -> Node {
    node(json!({
        "type": "frame", "id": "frame00001", "name": "Card", "width": 200, "height": "hug",
        "stroke": {"paints": [{"type": "solid", "color": "#000000"}], "width": 1},
        "children": [{"type": "rect", "id": "rect000001", "name": "Bg", "width": 10, "height": 10}]
    }))
}

#[test]
fn a_patch_sets_properties_and_keeps_the_rest() {
    let next = frame()
        .patched(&patch(json!({"name": "Hero", "opacity": 0.5})))
        .expect("valid");
    assert_eq!(next.name(), "Hero");
    assert_eq!(
        serde_json::to_value(next.opacity()).expect("ser"),
        json!(0.5)
    );
    assert_eq!(next.children().len(), 1, "children are kept");
    assert_eq!(next.id(), frame().id());
}

#[test]
fn null_puts_a_property_back_to_its_default() {
    let next = frame()
        .patched(&patch(json!({"stroke": null})))
        .expect("valid");
    assert!(next.stroke().is_none());
}

#[test]
fn a_patch_that_leaves_an_invalid_node_names_the_property() {
    let error = frame()
        .patched(&patch(json!({"name": "Fine", "opacity": "half"})))
        .expect_err("opacity is a number");
    assert_eq!(error.property, "opacity");
    assert!(!error.message.is_empty());

    let error = frame()
        .patched(&patch(json!({"width": null})))
        .expect_err("a width is required");
    assert_eq!(error.property, "width");

    let error = frame()
        .patched(&patch(json!({"content": []})))
        .expect_err("a frame has no content");
    assert_eq!(error.property, "content");
}

#[test]
fn variables_referred_to_are_found_anywhere_but_metadata() {
    let value = node(json!({
        "type": "frame", "id": "frame00001", "name": "Card",
        "width": {"$var": "width00001"}, "height": "hug",
        "opacity": {"$var": "opacity001"},
        "metadata": {"note": {"$var": "notreal001"}},
        "children": [{"type": "rect", "id": "rect000001", "name": "Bg", "width": 10, "height": 10,
                      "fills": [{"type": "solid", "color": {"$var": "color00001"}}]}]
    }));
    let found: Vec<VariableId> = referenced_variables(&value).into_iter().collect();
    let names: Vec<&str> = found.iter().map(VariableId::as_str).collect();
    assert_eq!(names, ["color00001", "opacity001", "width00001"]);
}
