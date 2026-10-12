//! Components, props, variants and instances (ARCHITECTURE §4.1 and §4.4). A component is a node tree with
//! typed props and variants. An instance points at a component and carries its own variant choice, prop
//! values, overrides by node id path and slot content. Resolving them (Prompt 17) is not this file's job:
//! every shape here must read and print back exactly as it was written.

use std::collections::BTreeSet;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value as Json, json};
use vellum_types::{
    Component, NODE_PROPERTIES, Node, NodePatch, Override, PropDef, Scalar, Variant,
};

fn outcome<T: DeserializeOwned>(input: Json) -> Result<T, serde_json::Error> {
    serde_json::from_value(input)
}

fn canonical<T: Serialize + DeserializeOwned>(input: Json) -> T {
    let value: T = outcome(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"));
    let back = serde_json::to_value(&value).expect("serialise");
    assert_eq!(back, input, "did not print back as it was read");
    value
}

fn button_root() -> Json {
    json!({
        "type": "frame",
        "id": "btn000001",
        "name": "Button",
        "width": "hug",
        "height": "hug",
        "layout": "row",
        "gap": 8.0,
        "padding": 12.0,
        "fills": [{"type": "solid", "color": "#1A2B3C"}],
        "children": [
            {
                "type": "text",
                "id": "lbl000001",
                "name": "Label",
                "width": "hug",
                "height": "hug",
                "content": [{"text": "Save", "font": "Geist", "size": 14.0}]
            },
            {
                "type": "icon",
                "id": "ico000001",
                "name": "Icon",
                "width": 16.0,
                "height": 16.0,
                "library": "lucide",
                "icon": "arrow-right",
                "weight": 400
            }
        ]
    })
}

#[test]
fn a_component_has_a_name_a_root_tree_and_typed_props() {
    canonical::<Component>(json!({
        "name": "Button",
        "props": {
            "count": {"type": "number", "default": 3.0},
            "label": {"type": "text", "default": "Save"},
            "showIcon": {"type": "boolean", "default": true}
        },
        "root": button_root()
    }));
}

#[test]
fn a_variant_is_a_set_of_overrides_on_the_component_tree() {
    canonical::<Variant>(json!({
        "properties": {"State": "hover", "Size": "large"},
        "overrides": {
            "btn000001": {"props": {"fills": [{"type": "solid", "color": "#E0E8F0"}]}},
            "btn000001/ico000001": {"props": {"visible": false}}
        }
    }));
}

#[test]
fn a_component_with_variants_keeps_them_in_order() {
    let component = canonical::<Component>(json!({
        "name": "Button",
        "root": button_root(),
        "variants": [
            {"properties": {"State": "default"}},
            {"properties": {"State": "hover"}, "overrides": {"btn000001": {"props": {"gap": 4.0}}}}
        ]
    }));
    let states: Vec<&str> = component
        .variants
        .iter()
        .map(|variant| variant.properties["State"].as_str())
        .collect();
    assert_eq!(states, ["default", "hover"]);
}

#[test]
fn a_prop_is_a_boolean_text_or_number_with_a_default_of_its_own_type() {
    for ok in [
        json!({"type": "boolean", "default": true}),
        json!({"type": "text", "default": "Save"}),
        json!({"type": "number", "default": 3.0}),
    ] {
        canonical::<PropDef>(ok);
    }
    for bad in [
        json!({"type": "text", "default": 3.0}),
        json!({"type": "number", "default": "3"}),
        json!({"type": "boolean", "default": 1.0}),
        json!({"type": "toggle", "default": true}),
        json!({"type": "text", "default": "Save", "extra": 1}),
        json!({"default": "Save"}),
    ] {
        assert!(
            outcome::<PropDef>(bad.clone()).is_err(),
            "{bad} should be refused"
        );
    }
}

#[test]
fn an_instance_prop_value_is_a_literal_boolean_number_or_text() {
    assert_eq!(
        outcome::<Scalar>(json!(true)).expect("boolean"),
        Scalar::Bool(true)
    );
    assert_eq!(
        outcome::<Scalar>(json!(2.5)).expect("number"),
        Scalar::Number(2.5)
    );
    assert_eq!(
        outcome::<Scalar>(json!("Send")).expect("text"),
        Scalar::Text("Send".to_owned())
    );
    assert!(
        outcome::<Scalar>(json!({"$var": "va000001"})).is_err(),
        "a prop value is a literal, not a reference"
    );
    assert!(outcome::<Scalar>(json!(null)).is_err());
}

#[test]
fn an_instance_carries_its_variant_props_overrides_and_slot_content() {
    canonical::<Node>(json!({
        "type": "instance",
        "id": "inst000001",
        "name": "Save",
        "width": "hug",
        "height": "hug",
        "componentId": "comp000001",
        "variant": {"State": "hover"},
        "props": {"count": 2.0, "label": "Send", "showIcon": false},
        "overrides": {
            "btn000001/lbl000001": {
                "props": {"content": [{"text": "Send", "font": "Geist", "size": 14.0}]}
            },
            "btn000001/ico000001": {
                "replace": {
                    "type": "icon",
                    "id": "ico000002",
                    "name": "Send icon",
                    "width": 16.0,
                    "height": 16.0,
                    "library": "lucide",
                    "icon": "send",
                    "weight": 400
                }
            },
            "btn000001": {
                "children": [
                    {"type": "rect", "id": "rect000001", "name": "Spacer", "width": "fill", "height": 8.0}
                ]
            }
        },
        "slotContent": {
            "slot000001": [
                {"type": "rect", "id": "rect000002", "name": "Filler", "width": "fill", "height": 8.0}
            ]
        }
    }));
}

#[test]
fn an_override_is_exactly_one_of_props_replace_or_children() {
    assert!(outcome::<Override>(json!({"props": {"visible": false}})).is_ok());
    assert!(outcome::<Override>(json!({
        "replace": {"type": "rect", "id": "rect000001", "name": "r", "width": 1.0, "height": 1.0}
    }))
    .is_ok());
    assert!(outcome::<Override>(json!({"children": []})).is_ok());
    assert!(
        outcome::<Override>(json!({})).is_err(),
        "an override says what it does"
    );
    assert!(outcome::<Override>(json!({"props": {}, "children": []})).is_err());
    assert!(
        outcome::<Override>(json!({"fills": []})).is_err(),
        "a bare patch is not an override"
    );
    assert!(outcome::<Override>(json!({"children": "none"})).is_err());
}

#[test]
fn a_props_override_sets_properties_that_some_node_has() {
    for allowed in [
        json!({"fills": []}),
        json!({"name": "Renamed"}),
        json!({"componentId": "comp000002"}),
        json!({"visible": {"$var": "va000001"}}),
        json!({"content": []}),
    ] {
        outcome::<Override>(json!({"props": allowed.clone()}))
            .unwrap_or_else(|e| panic!("{allowed} should be allowed: {e}"));
    }
    let message = outcome::<Override>(json!({"props": {"fils": []}}))
        .expect_err("a misspelt property is refused")
        .to_string();
    assert!(message.contains("fils"), "{message}");
}

#[test]
fn a_props_override_cannot_set_the_id_the_type_or_the_children() {
    for refused in ["id", "type", "children"] {
        assert!(
            outcome::<Override>(json!({"props": {refused: "x"}})).is_err(),
            "{refused} should be refused in a props override"
        );
    }
    assert!(
        NodePatch::new(std::collections::BTreeMap::from([(
            "children".to_owned(),
            json!([])
        )]))
        .is_err()
    );
}

/// Every object in the schema that describes a node (it has an `id` and a `width`) contributes its property
/// names, the `type` tag excepted. The node union inlines each kind's properties, so the walk looks at
/// every nested object rather than at fixed places.
fn collect_node_properties(schema: &Json, names: &mut BTreeSet<String>) {
    match schema {
        Json::Object(map) => {
            if let Some(Json::Object(properties)) = map.get("properties")
                && properties.contains_key("id")
                && properties.contains_key("width")
            {
                names.extend(properties.keys().filter(|k| *k != "type").cloned());
            }
            for child in map.values() {
                collect_node_properties(child, names);
            }
        }
        Json::Array(items) => {
            for item in items {
                collect_node_properties(item, names);
            }
        }
        _ => {}
    }
}

#[test]
fn the_node_property_list_is_the_one_the_node_schemas_describe() {
    // The properties a props override may set are the ones every node schema describes (ARCHITECTURE §4.1).
    // The list in the crate must match them exactly, in both directions.
    let schema = serde_json::to_value(schemars::schema_for!(Node)).expect("serialise");
    let mut from_schema = BTreeSet::new();
    collect_node_properties(&schema, &mut from_schema);
    let listed: BTreeSet<String> = NODE_PROPERTIES
        .iter()
        .map(|name| name.to_string())
        .collect();
    let only_listed: Vec<&String> = listed.difference(&from_schema).collect();
    let only_schema: Vec<&String> = from_schema.difference(&listed).collect();
    assert!(
        only_listed.is_empty() && only_schema.is_empty(),
        "listed but not in the schema: {only_listed:?}; in the schema but not listed: {only_schema:?}"
    );
    assert!(
        from_schema.len() > 40,
        "the walk found only {} properties",
        from_schema.len()
    );
}
