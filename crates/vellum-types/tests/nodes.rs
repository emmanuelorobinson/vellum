//! The node union (ARCHITECTURE §4.1): every kind parses, prints back to the same JSON, and refuses
//! fields it does not know. Only `metadata` is free-form.

use serde_json::{Value as Json, json};
use vellum_types::{BlendMode, Node, NodeKind, Point, Position, Size, Value};

/// Parses `input` as a node and checks that printing it gives `input` back, key for key. The inputs
/// are written in canonical form: a field at its default is left out.
fn canonical(input: Json) -> Node {
    let node: Node =
        serde_json::from_value(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"));
    let back = serde_json::to_value(&node).expect("serialise");
    assert_eq!(back, input, "the node did not print back as it was read");
    node
}

fn rejected(input: Json) -> String {
    serde_json::from_value::<Node>(input.clone())
        .err()
        .unwrap_or_else(|| panic!("{input} should have been rejected"))
        .to_string()
}

fn base(kind: &str) -> serde_json::Map<String, Json> {
    let Json::Object(map) = json!({
        "type": kind,
        "id": "node000001",
        "name": "Thing",
        "width": 100.0,
        "height": 50.0
    }) else {
        unreachable!()
    };
    map
}

fn with(kind: &str, extra: Json) -> Json {
    let mut map = base(kind);
    let Json::Object(extra) = extra else {
        panic!("extra must be an object")
    };
    map.extend(extra);
    Json::Object(map)
}

#[test]
fn there_are_eighteen_node_kinds() {
    assert_eq!(NodeKind::ALL.len(), 18);
    let names: Vec<&str> = NodeKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        names,
        [
            "frame", "group", "rect", "ellipse", "polygon", "star", "line", "vector", "boolean",
            "text", "image", "icon", "instance", "script", "note", "context", "prompt", "embed"
        ]
    );
}

#[test]
fn a_minimal_node_takes_the_documented_defaults() {
    let node = canonical(Json::Object(base("rect")));
    assert_eq!(node.kind(), NodeKind::Rect);
    assert_eq!(node.id().as_str(), "node000001");
    assert_eq!(node.name(), "Thing");
    assert_eq!(node.width(), &Size::Fixed(Value::Literal(100.0)));
    assert_eq!(node.height(), &Size::Fixed(Value::Literal(50.0)));
    assert_eq!(node.visible(), &Value::Literal(true));
    assert!(!node.locked());
    assert_eq!(node.opacity(), &Value::Literal(1.0));
    assert_eq!(node.rotation(), &Value::Literal(0.0));
    assert!(!node.flip_x() && !node.flip_y());
    assert_eq!(node.position(), &Position::Auto);
    assert!(node.fills().is_empty());
    assert!(node.stroke().is_none());
    assert!(node.effects().is_empty());
    assert_eq!(node.blend_mode(), BlendMode::Normal);
    assert!(node.agent_notes().is_none());
    assert!(node.prototype().is_none());
    assert!(node.metadata().is_empty());
    assert!(node.children().is_empty());
}

#[test]
fn common_fields_round_trip() {
    let node = canonical(with(
        "rect",
        json!({
            "visible": {"$var": "va000001"},
            "locked": true,
            "opacity": 0.5,
            "rotation": 90.0,
            "flipX": true,
            "flipY": true,
            "position": {"type": "absolute", "x": 10.0, "y": -4.5},
            "width": {"fill": 20.0},
            "height": "hug",
            "fills": [{"type": "solid", "color": "#FF0000"}],
            "stroke": {"paints": [{"type": "solid", "color": "#000000"}], "width": 2.0},
            "effects": [{"type": "layer-blur", "radius": 4.0}],
            "blendMode": "multiply",
            "agentNotes": "Keep this square.",
            "prototype": {"target": "frame00002", "transition": {"type": "dissolve", "durationMs": 200}},
            "metadata": {"source": "src/Card.tsx", "nested": {"a": [1, 2, {"b": null}]}}
        }),
    ));
    assert_eq!(
        node.position(),
        &Position::Absolute(Point { x: 10.0, y: -4.5 })
    );
    assert_eq!(node.blend_mode(), BlendMode::Multiply);
    assert_eq!(node.metadata().len(), 2);
}

#[test]
fn a_frame_carries_layout_clip_theme_and_a_slot() {
    let node = canonical(with(
        "frame",
        json!({
            "layout": "grid",
            "columns": 3,
            "rows": 2,
            "gap": 8.0,
            "padding": [8.0, 16.0],
            "justify": "space-between",
            "align": "center",
            "layoutIncludesStroke": true,
            "clip": true,
            "cornerRadius": 12.0,
            "theme": {"Mode": "Dark"},
            "slot": {"suggested": ["comp000001", "comp000002"]},
            "children": [
                with("rect", json!({"id": "child000001"})),
                with("group", json!({
                    "id": "child000002",
                    "children": [with("line", json!({"id": "child000003"}))]
                }))
            ]
        }),
    ));
    assert_eq!(node.kind(), NodeKind::Frame);
    assert_eq!(node.children().len(), 2);
    assert_eq!(node.children()[1].children()[0].kind(), NodeKind::Line);
}

#[test]
fn padding_takes_one_two_or_four_values_and_corner_radius_one_or_four() {
    canonical(with("frame", json!({"padding": 8.0})));
    canonical(with("frame", json!({"padding": [8.0, 16.0]})));
    canonical(with("frame", json!({"padding": [1.0, 2.0, 3.0, 4.0]})));
    canonical(with("frame", json!({"padding": {"$var": "va000002"}})));
    canonical(with(
        "frame",
        json!({"padding": [{"$var": "va000002"}, 4.0]}),
    ));
    canonical(with("rect", json!({"cornerRadius": [1.0, 2.0, 3.0, 4.0]})));
    for bad in [
        json!([]),
        json!([1.0]),
        json!([1.0, 2.0, 3.0]),
        json!([1.0, 2.0, 3.0, 4.0, 5.0]),
    ] {
        rejected(with("frame", json!({"padding": bad})));
    }
    rejected(with("rect", json!({"cornerRadius": [1.0, 2.0]})));
}

#[test]
fn every_other_shape_kind_round_trips() {
    canonical(with(
        "group",
        json!({"children": [with("rect", json!({"id": "child000001"}))]}),
    ));
    canonical(with("rect", json!({"cornerRadius": 6.0})));
    canonical(with(
        "ellipse",
        json!({"innerRadius": 0.6, "startAngle": 45.0, "sweepAngle": 270.0}),
    ));
    canonical(Json::Object(base("ellipse")));
    canonical(with("polygon", json!({"sides": 6, "cornerRadius": 2.0})));
    canonical(with("star", json!({"points": 5, "innerRatio": 0.4})));
    canonical(Json::Object(base("line")));
    canonical(with(
        "vector",
        json!({
            "path": "M0 0 L10 0 L10 10 Z",
            "viewBox": {"x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0},
            "fillRule": "evenodd"
        }),
    ));
    canonical(with(
        "vector",
        json!({"path": "M0 0", "viewBox": {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}}),
    ));
    canonical(with(
        "boolean",
        json!({"op": "subtract", "children": [
            with("rect", json!({"id": "child000001"})),
            with("ellipse", json!({"id": "child000002"}))
        ]}),
    ));
}

#[test]
fn ellipse_defaults_make_a_full_ellipse() {
    let Node::Ellipse(ellipse) = canonical(Json::Object(base("ellipse"))) else {
        panic!("not an ellipse")
    };
    assert_eq!(ellipse.inner_radius, 0.0);
    assert_eq!(ellipse.start_angle, 0.0);
    assert_eq!(ellipse.sweep_angle, 360.0);
}

#[test]
fn text_has_rich_runs_alignment_and_resize() {
    let node = canonical(with(
        "text",
        json!({
            "content": [
                {"text": "Hello ", "font": "Inter", "size": 16.0},
                {
                    "text": "world",
                    "font": "Inter",
                    "weight": 700,
                    "style": "italic",
                    "size": {"$var": "va000003"},
                    "lineHeight": 24.0,
                    "letterSpacing": 0.2,
                    "underline": true,
                    "strikethrough": true,
                    "link": "https://example.com"
                }
            ],
            "align": "justify",
            "verticalAlign": "middle",
            "resize": "auto-height",
            "textStyle": "style00001"
        }),
    ));
    assert_eq!(node.kind(), NodeKind::Text);
}

#[test]
fn text_defaults_are_left_top_and_fixed() {
    let Node::Text(text) = canonical(with("text", json!({"content": []}))) else {
        panic!("not text")
    };
    assert_eq!(text.align.as_str(), "left");
    assert_eq!(text.vertical_align.as_str(), "top");
    assert_eq!(text.resize.as_str(), "fixed");
}

#[test]
fn image_icon_and_instance_round_trip() {
    canonical(with("image", json!({"asset": "a1b2c3d4e5", "fit": "fit"})));
    canonical(with(
        "image",
        json!({"asset": "a1b2c3d4e5", "crop": [1.0, 0.0, 0.0, 1.0, -10.0, -4.0]}),
    ));
    canonical(with(
        "icon",
        json!({"library": "lucide", "icon": "arrow-right", "weight": 400, "fill": true}),
    ));
    canonical(with(
        "icon",
        json!({"library": "lucide", "icon": {"$var": "va000004"}, "weight": 400}),
    ));
    canonical(with(
        "instance",
        json!({
            "componentId": "comp000001",
            "variant": {"Size": "Large"},
            "props": {"label": "Save", "count": 3.0},
            "overrides": {"aaaaaaaaaa/bbbbbbbbbb": {"props": {"fills": []}}},
            "slotContent": {"slot000001": [with("rect", json!({"id": "child000001"}))]}
        }),
    ));
    canonical(with("instance", json!({"componentId": "comp000001"})));
}

#[test]
fn script_note_context_prompt_and_embed_round_trip() {
    canonical(with(
        "script",
        json!({
            "src": "scripts/grid.js",
            "inputs": {"columns": 3.0, "label": "Hi", "dense": false, "gap": {"$var": "va000002"}},
            "clip": true
        }),
    ));
    canonical(with("script", json!({"src": "scripts/grid.js"})));
    canonical(with(
        "note",
        json!({"content": "Check the contrast.", "color": "#FFE066"}),
    ));
    canonical(with(
        "context",
        json!({"content": "Use the primary button.", "pointsAt": "frame00002"}),
    ));
    canonical(with(
        "context",
        json!({"content": "Applies to this frame."}),
    ));
    canonical(with(
        "prompt",
        json!({"content": "Add a pricing section.", "engine": "claude-code", "model": "opus"}),
    ));
    canonical(with("prompt", json!({"content": "Add a pricing section."})));
    canonical(with(
        "embed",
        json!({
            "url": "http://localhost:3000",
            "device": "phone",
            "zoom": 0.5,
            "scroll": {"x": 0.0, "y": 120.0},
            "radius": 8.0
        }),
    ));
    canonical(with("embed", json!({"url": "http://localhost:3000"})));
}

#[test]
fn a_script_node_has_no_saved_children() {
    // Children of a script are derived output: rendered, never saved (ARCHITECTURE §4.1).
    let message = rejected(with("script", json!({"src": "a.js", "children": []})));
    assert!(message.contains("children"), "{message}");
}

#[test]
fn unknown_fields_are_refused() {
    assert!(rejected(with("rect", json!({"colour": "red"}))).contains("colour"));
    assert!(rejected(with("frame", json!({"gapp": 1.0}))).contains("gapp"));
    // A field that belongs to another kind is unknown here.
    assert!(rejected(with("rect", json!({"sides": 5}))).contains("sides"));
    // Nested objects are strict too.
    rejected(with(
        "rect",
        json!({"fills": [{"type": "solid", "color": "#000000", "extra": 1}]}),
    ));
    rejected(with(
        "rect",
        json!({"stroke": {"paints": [], "width": 1.0, "extra": 1}}),
    ));
    rejected(with(
        "rect",
        json!({"position": {"type": "absolute", "x": 1.0, "y": 1.0, "z": 1.0}}),
    ));
}

#[test]
fn metadata_is_the_one_free_form_field() {
    let node = canonical(with(
        "rect",
        json!({"metadata": {"anything": [1, "two", {"three": null}]}}),
    ));
    assert_eq!(
        node.metadata()["anything"],
        json!([1, "two", {"three": null}])
    );
}

#[test]
fn missing_required_fields_are_errors() {
    for field in ["id", "name", "width", "height"] {
        let mut map = base("rect");
        map.remove(field);
        let message = rejected(Json::Object(map));
        assert!(message.contains(field), "{field}: {message}");
    }
    rejected(with("vector", json!({"path": "M0 0"})));
    rejected(with("polygon", json!({})));
    rejected(with("star", json!({"points": 5})));
    rejected(with("image", json!({})));
    rejected(with("instance", json!({})));
}

#[test]
fn an_unknown_node_type_is_an_error() {
    rejected(with("circle", json!({})));
    let mut map = base("rect");
    map.remove("type");
    rejected(Json::Object(map));
}

#[test]
fn ids_inside_nodes_are_validated() {
    rejected(with("rect", json!({"id": "bad/id"})));
    rejected(with("frame", json!({"slot": {"suggested": ["bad/id"]}})));
    rejected(with("instance", json!({"componentId": ""})));
}

#[test]
fn a_word_has_one_spelling_not_two() {
    // serde's derive reads {"word": null} as the unit variant "word"; the file must not.
    rejected(with("rect", json!({"blendMode": {"multiply": null}})));
    rejected(with("frame", json!({"layout": {"row": null}})));
    rejected(with("rect", json!({"width": {"hug": null}})));
    rejected(with("rect", json!({"blendMode": ["multiply"]})));
    rejected(with("rect", json!({"blendMode": "Multiply"})));
}

#[test]
fn a_document_nested_too_deeply_is_an_error_not_a_crash() {
    fn chain(depth: usize) -> String {
        let mut text = String::from(
            r#"{"type":"line","id":"leaf000001","name":"x","width":1.0,"height":1.0}"#,
        );
        for i in 0..depth {
            text = format!(
                r#"{{"type":"group","id":"g{i:09}","name":"x","width":1.0,"height":1.0,"children":[{text}]}}"#
            );
        }
        text
    }
    // A test thread has 2 MiB of stack, as a runtime worker does.
    let outcome = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let shallow = serde_json::from_str::<Node>(&chain(20));
            let absurd = serde_json::from_str::<Node>(&chain(2000));
            (shallow.is_ok(), absurd.is_err())
        })
        .expect("spawn")
        .join()
        .expect("deserialising must not overflow the stack");
    assert_eq!(outcome, (true, true));
}

#[test]
fn the_node_union_has_a_json_schema() {
    // Prompt 8 publishes the schema; here it must at least build for the recursive union and for the
    // types written by hand (ids, colours, sizes), and describe them as the file spells them.
    let schema = serde_json::to_value(schemars::schema_for!(Node)).expect("serialise schema");
    let text = schema.to_string();
    for kind in NodeKind::ALL {
        assert!(
            text.contains(&format!("\"{}\"", kind.as_str())),
            "no {} in the schema",
            kind.as_str()
        );
    }
    assert!(
        text.contains("\"$var\""),
        "the variable reference is missing"
    );
    assert!(
        text.contains("\"hug\"") && text.contains("\"fill\""),
        "the size keywords are missing"
    );
    assert!(
        text.contains("color-burn"),
        "the kebab-case blend modes are missing"
    );
}
