//! TEST DATA: a small design system and a card that uses it.
use serde_json::{Value, json};
use vellum_types::{Document, NodeId};

use super::*;
use crate::ports::SpecFormat;

fn node_id(id: &str) -> NodeId {
    NodeId::new(id).unwrap()
}

fn document() -> Document {
    serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA", "createdWith": "test"},
        "themes": {"Mode": ["Light", "Dark"]},
        "variables": {
            "v-surface": {"type": "color", "name": "color/surface", "values": [
                {"value": "#FFFFFF"}, {"value": "#101014", "when": {"Mode": "Dark"}}]},
            "v-card": {"type": "color", "name": "color/card", "values": [{"$alias": "v-surface"}]},
            "v-text": {"type": "color", "name": "color/text", "values": [{"value": "#111111"}]},
            "v-gap": {"type": "number", "name": "space/3", "values": [{"value": 12}]},
            "v-size": {"type": "number", "name": "text/body", "values": [{"value": 16}]},
            "v-unused": {"type": "number", "name": "space/unused", "values": [{"value": 1}]},
        },
        "styles": {
            "st-body": {"type": "text", "name": "Body", "font": "Inter", "size": {"$var": "v-size"}},
            "st-unused": {"type": "text", "name": "Unused", "font": "Inter", "size": 40},
        },
        "components": {
            "co-button": {
                "name": "Button",
                "props": {"label": {"type": "text", "default": "Go"}},
                "root": {"type": "frame", "id": "button-root", "name": "Button", "width": "hug",
                    "height": 40, "padding": [0, 16], "layout": "row",
                    "fills": [{"type": "solid", "color": {"$var": "v-text"}}]},
                "variants": [],
            }
        },
        "pages": [{"id": "page-1", "name": "Page 1", "children": [
            {"type": "frame", "id": "card", "name": "Card", "width": 320, "height": "hug",
             "layout": "column", "gap": {"$var": "v-gap"}, "padding": 16, "locked": true,
             "agentNotes": "Keep the title one line.",
             "fills": [{"type": "solid", "color": {"$var": "v-card"}}],
             "children": [
                {"type": "text", "id": "title", "name": "Title", "width": "fill", "height": "hug",
                 "textStyle": "st-body",
                 "content": [{"text": "Hello", "font": "Inter", "size": {"$var": "v-size"}}]},
                {"type": "instance", "id": "go", "name": "Go", "width": "hug", "height": "hug",
                 "componentId": "co-button", "props": {"label": "Open"}},
                {"type": "rect", "id": "rule", "name": "Rule", "width": "fill", "height": 1,
                 "position": {"type": "absolute", "x": 0, "y": 40}},
             ]},
            {"type": "frame", "id": "other", "name": "Other", "width": 10, "height": 10},
        ]}],
    }))
    .unwrap()
}

fn build() -> Value {
    LayoutSpecV1.build(&document(), &node_id("card")).unwrap()
}

fn names(spec: &Value, list: &str) -> Vec<String> {
    spec[list]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_spec_names_its_format_and_the_layer() {
    let spec = build();
    assert_eq!(spec["format"], "layout-spec-v1");
    assert_eq!(
        spec["frame"],
        json!({"id": "card", "name": "Card", "page": "page-1"})
    );
    assert_eq!(spec["themes"], json!({"Mode": ["Light", "Dark"]}));
    assert_eq!(spec["root"]["type"], "frame");
    assert_eq!(spec["root"]["layout"], "column");
    assert_eq!(spec["root"]["width"], 320);
    assert_eq!(spec["root"]["height"], "hug");
    assert_eq!(spec, build(), "the same layer gives the same spec");
}

#[test]
fn variables_become_tokens_by_name() {
    let spec = build();
    assert_eq!(spec["root"]["gap"], json!({"$token": "space/3"}));
    assert_eq!(
        spec["root"]["fills"][0]["color"],
        json!({"$token": "color/card"})
    );
    assert!(
        !spec.to_string().contains("$var"),
        "no variable id is left: {spec}"
    );
}

#[test]
fn the_token_list_holds_what_is_used_and_what_it_leads_to() {
    let spec = build();
    // The card's own, the title's, the button definition's, the style's; the alias's target; nothing else.
    assert_eq!(
        names(&spec, "tokens"),
        [
            "color/card",
            "color/surface",
            "color/text",
            "space/3",
            "text/body"
        ]
    );
    let tokens = spec["tokens"].as_array().unwrap();
    let get = |name: &str| tokens.iter().find(|t| t["name"] == name).unwrap();
    assert_eq!(get("color/card")["type"], "color");
    assert_eq!(
        get("color/card")["value"],
        json!({"$token": "color/surface"})
    );
    assert_eq!(get("color/surface")["value"], "#FFFFFF");
    assert_eq!(
        get("color/surface")["modes"],
        json!({"Mode: Dark": "#101014"})
    );
    assert_eq!(get("space/3")["value"], 12);
    assert!(
        get("space/3").get("modes").is_none(),
        "no modes when there are none"
    );
}

#[test]
fn children_keep_their_order_and_what_makes_them_different() {
    let spec = build();
    let children = spec["root"]["children"].as_array().unwrap();
    let ids: Vec<_> = children.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["title", "go", "rule"]);
    assert_eq!(
        children[2]["position"],
        json!({"type": "absolute", "x": 0, "y": 40})
    );
    assert_eq!(children[0]["width"], "fill");
    assert_eq!(spec["root"]["notes"], "Keep the title one line.");
    assert!(spec["root"].get("agentNotes").is_none());
    assert!(
        spec["root"].get("locked").is_none(),
        "locking is for the canvas"
    );
}

#[test]
fn text_styles_are_listed_once_used() {
    let spec = build();
    assert_eq!(spec["root"]["children"][0]["textStyleName"], "Body");
    let styles = spec["styles"].as_object().unwrap();
    assert_eq!(styles.keys().collect::<Vec<_>>(), ["st-body"]);
    assert_eq!(styles["st-body"]["size"], json!({"$token": "text/body"}));
    assert_eq!(styles["st-body"]["name"], "Body");
}

#[test]
fn components_are_listed_with_their_props_and_definition() {
    let spec = build();
    let go = &spec["root"]["children"][1];
    assert_eq!(go["component"], "Button");
    assert_eq!(go["props"], json!({"label": "Open"}));
    let components = spec["components"].as_array().unwrap();
    assert_eq!(components.len(), 1);
    assert_eq!(components[0]["id"], "co-button");
    assert_eq!(components[0]["name"], "Button");
    assert_eq!(components[0]["instances"], 1);
    assert_eq!(components[0]["props"]["label"]["default"], "Go");
    assert_eq!(
        components[0]["definition"]["fills"][0]["color"],
        json!({"$token": "color/text"})
    );
}

#[test]
fn a_layer_that_is_not_there_is_refused() {
    let error = LayoutSpecV1
        .build(&document(), &node_id("nope"))
        .unwrap_err();
    assert!(error.to_string().contains("nope"), "{error}");
}

#[test]
fn a_variable_that_is_gone_is_said_not_invented() {
    let mut doc = document();
    doc.variables
        .remove(&vellum_types::VariableId::new("v-gap").unwrap());
    let spec = LayoutSpecV1.build(&doc, &node_id("card")).unwrap();
    assert_eq!(spec["root"]["gap"], json!({"$unresolved": "v-gap"}));
    assert!(!names(&spec, "tokens").contains(&"space/3".to_owned()));
}

#[test]
fn a_spec_can_start_below_the_top() {
    let spec = LayoutSpecV1.build(&document(), &node_id("title")).unwrap();
    assert_eq!(spec["root"]["id"], "title");
    assert_eq!(spec["frame"]["page"], "page-1");
    assert_eq!(names(&spec, "tokens"), ["text/body"]);
    assert!(spec["components"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// Stack profiles

#[test]
fn the_three_stacks_ship_as_data() {
    let stacks = StackProfiles::builtin();
    let ids: Vec<_> = stacks.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["shadcn", "react-native", "swiftui"]);
    for profile in stacks.iter() {
        assert!(
            !profile.label.is_empty() && !profile.language.is_empty(),
            "{}",
            profile.id
        );
        assert!(
            !profile.summary.is_empty() && !profile.rules.is_empty(),
            "{}",
            profile.id
        );
        for key in ["frame", "text", "image", "icon", "instance"] {
            assert!(
                profile.elements.contains_key(key),
                "{}: element {key}",
                profile.id
            );
        }
        for key in [
            "row", "column", "grid", "none", "gap", "padding", "justify", "align", "fill", "hug",
            "fixed",
        ] {
            assert!(
                profile.layout.contains_key(key),
                "{}: layout {key}",
                profile.id
            );
        }
        for key in ["reference", "naming", "modes"] {
            assert!(
                profile.tokens.contains_key(key),
                "{}: tokens {key}",
                profile.id
            );
        }
        assert!(!profile.components.is_empty(), "{}", profile.id);
    }
}

#[test]
fn a_stack_is_found_by_id_and_an_unknown_one_lists_the_others() {
    let stacks = StackProfiles::builtin();
    assert_eq!(stacks.get("swiftui").unwrap().language, "swift");
    let error = stacks.require("flutter").unwrap_err();
    assert!(error.contains("shadcn, react-native, swiftui"), "{error}");
}

#[test]
fn a_stack_round_trips_through_json_and_refuses_what_is_incomplete() {
    let stacks = StackProfiles::builtin();
    for profile in stacks.iter() {
        assert_eq!(
            &StackProfile::from_json(&profile.to_json()).unwrap(),
            profile
        );
    }
    let mut broken = stacks.get("shadcn").unwrap().to_json();
    broken.as_object_mut().unwrap().remove("rules");
    assert!(
        StackProfile::from_json(&broken)
            .unwrap_err()
            .contains("rules")
    );
    let mut broken = stacks.get("shadcn").unwrap().to_json();
    broken["layout"]["gap"] = json!(3);
    assert!(
        StackProfile::from_json(&broken)
            .unwrap_err()
            .contains("layout.gap")
    );
    assert!(StackProfile::from_json(&json!([])).is_err());
}

#[test]
fn a_person_can_add_a_stack_or_replace_one() {
    let mut custom = StackProfiles::builtin().get("shadcn").unwrap().clone();
    custom.id = "flutter".into();
    custom.label = "Flutter".into();
    let mut stacks = StackProfiles::builtin().with(custom);
    assert_eq!(stacks.iter().count(), 4);
    assert_eq!(stacks.get("flutter").unwrap().label, "Flutter");
    let mut replaced = stacks.get("swiftui").unwrap().clone();
    replaced.summary = "TEST DATA".into();
    stacks = stacks.with(replaced);
    assert_eq!(stacks.iter().count(), 4, "same id replaces");
    assert_eq!(stacks.get("swiftui").unwrap().summary, "TEST DATA");
}

#[test]
fn stack_data_does_not_name_tools_the_spec_does_not_mention() {
    // The hints describe the spec's own words, so an agent can follow them without guessing.
    let words = ["fill", "hug", "gap", "padding", "justify", "align"];
    for profile in StackProfiles::builtin().iter() {
        for word in words {
            assert!(profile.layout.contains_key(word), "{}: {word}", profile.id);
        }
    }
}
