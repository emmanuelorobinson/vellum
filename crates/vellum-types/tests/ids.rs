//! Typed ids (guide §9 Identifiers): no raw strings across crate boundaries, no `/` in an id,
//! and a node id path that prints and parses back to itself.

use std::str::FromStr;

use vellum_types::{AssetId, ComponentId, NodeId, NodeIdPath, PageId, StyleId, VariableId};

#[test]
fn an_id_round_trips_through_text_and_json() {
    let id = NodeId::new("aB3_-xY9zQ").expect("a ten character nanoid is valid");
    assert_eq!(id.as_str(), "aB3_-xY9zQ");
    assert_eq!(id.to_string(), "aB3_-xY9zQ");
    assert_eq!(NodeId::from_str("aB3_-xY9zQ").expect("parse"), id);

    let json = serde_json::to_string(&id).expect("serialise");
    assert_eq!(json, r#""aB3_-xY9zQ""#);
    assert_eq!(
        serde_json::from_str::<NodeId>(&json).expect("deserialise"),
        id
    );
}

#[test]
fn an_id_never_contains_a_slash() {
    assert!(NodeId::new("a/b").is_err());
    assert!(NodeId::new("/").is_err());
    assert!(serde_json::from_str::<NodeId>(r#""card/actions""#).is_err());
}

#[test]
fn an_id_is_not_empty_blank_or_outside_the_nanoid_alphabet() {
    for bad in ["", " ", "a b", "a.b", "a\"b", "é", "a\nb"] {
        assert!(NodeId::new(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn an_id_has_a_length_limit() {
    assert!(NodeId::new("a".repeat(128)).is_ok());
    assert!(NodeId::new("a".repeat(129)).is_err());
}

#[test]
fn every_kind_of_id_is_its_own_type() {
    // These lines compile only because the types differ; a NodeId cannot stand in for a PageId.
    let _: PageId = PageId::new("page000001").expect("page id");
    let _: VariableId = VariableId::new("var0000001").expect("variable id");
    let _: ComponentId = ComponentId::new("comp000001").expect("component id");
    let _: StyleId = StyleId::new("style00001").expect("style id");
    let _: AssetId = AssetId::new("a".repeat(64)).expect("a content hash is a valid asset id");
}

#[test]
fn a_node_id_path_prints_and_parses_back() {
    let path = NodeIdPath::from_str("card00000a/actions000/button0001").expect("parse");
    assert_eq!(path.len(), 3);
    assert_eq!(path.to_string(), "card00000a/actions000/button0001");
    assert_eq!(
        path.segments().map(NodeId::as_str).collect::<Vec<_>>(),
        ["card00000a", "actions000", "button0001"]
    );

    let json = serde_json::to_string(&path).expect("serialise");
    assert_eq!(json, r#""card00000a/actions000/button0001""#);
    assert_eq!(
        serde_json::from_str::<NodeIdPath>(&json).expect("deserialise"),
        path
    );
}

#[test]
fn a_node_id_path_of_one_segment_is_just_the_id() {
    let path = NodeIdPath::from_str("solo000001").expect("parse");
    assert_eq!(path.len(), 1);
    assert_eq!(path.to_string(), "solo000001");
}

#[test]
fn a_node_id_path_rejects_empty_segments() {
    for bad in ["", "/", "a/", "/a", "a//b"] {
        assert!(
            NodeIdPath::from_str(bad).is_err(),
            "{bad:?} should be rejected"
        );
    }
}

#[test]
fn a_node_id_path_can_be_built_from_ids_and_extended() {
    let a = NodeId::new("aaaaaaaaaa").expect("id");
    let b = NodeId::new("bbbbbbbbbb").expect("id");
    let path = NodeIdPath::from(a.clone()).join(b.clone());
    assert_eq!(path.to_string(), "aaaaaaaaaa/bbbbbbbbbb");
    assert_eq!(path.first(), &a);
    assert_eq!(path.last(), &b);
}

#[test]
fn a_node_id_path_works_as_a_json_object_key() {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    map.insert(
        NodeIdPath::from_str("aaaaaaaaaa/bbbbbbbbbb").expect("path"),
        1_u8,
    );
    let json = serde_json::to_string(&map).expect("serialise");
    assert_eq!(json, r#"{"aaaaaaaaaa/bbbbbbbbbb":1}"#);
    let back: BTreeMap<NodeIdPath, u8> = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, map);
}
