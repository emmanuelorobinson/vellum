//! Binding a property to a variable: the variable must exist and have a type the property takes. Rejected when
//! the binding is made (an op), checked again for a whole document, and a boolean variable drives visibility.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};
use vellum_types::VariableType as T;

use super::{axes, doc, frame, node, rect, theme};
use crate::ops::tests::common::{self as c, ALICE};
use crate::ops::{Op, OpErrorKind, check_document};
use crate::resolver::variables::{
    Resolver, VariableError, check_node_bindings, check_style_bindings, resolve_visible,
};

/// Variables of every type: `c1` colour, `n1` number, `s1` string, `b1` boolean.
fn variables() -> Json {
    json!({
        "c000000001": {"type": "color", "name": "c", "values": [{"value": "#112233"}]},
        "n000000001": {"type": "number", "name": "n", "values": [{"value": 2}]},
        "s000000001": {"type": "string", "name": "s", "values": [{"value": "x"}]},
        "b000000001": {"type": "boolean", "name": "b", "values": [{"value": true}]}
    })
}

fn bound() -> vellum_types::Document {
    doc(
        json!({"themes": axes(), "variables": variables()}),
        json!([]),
    )
}

fn var(id: &str) -> Json {
    json!({"$var": id})
}

/// The problem paths `check_node_bindings` finds in `value`.
fn problems(value: Json) -> Vec<(String, T)> {
    check_node_bindings(&bound(), &node(value))
        .into_iter()
        .map(|p| {
            let found = p.found().expect("a type problem");
            (p.at, found)
        })
        .collect()
}

#[test]
fn a_boolean_variable_drives_visibility() {
    let d = doc(
        json!({"themes": axes(), "variables": {
            "show000001": {"type": "boolean", "name": "flag/show", "values": [
                {"value": true}, {"value": false, "when": {"Mode": "Dark"}}
            ]}
        }}),
        json!([
            rect("light00001", json!({"visible": var("show000001")})),
            frame(
                "dark000001",
                json!({"theme": {"Mode": "Dark"}}),
                json!([rect("inner00001", json!({"visible": var("show000001")}))])
            ),
            rect("plain00001", json!({}))
        ]),
    );
    let tree = Resolver::new().resolve_page(&d, &d.pages[0]);
    assert!(tree[0].visible);
    assert!(
        !tree[1].children[0].visible,
        "the variable is false in Dark"
    );
    assert!(tree[2].visible);
    // The resolved node holds the literal, not the reference.
    assert_eq!(
        tree[1].children[0].node.visible(),
        &vellum_types::Value::Literal(false)
    );

    let hidden = node(rect("again00001", json!({"visible": var("show000001")})));
    assert_eq!(resolve_visible(&d, &hidden, &theme(&d, &[])), Ok(true));
    assert_eq!(
        resolve_visible(&d, &hidden, &theme(&d, &[("Mode", "Dark")])),
        Ok(false)
    );
}

#[test]
fn a_flag_that_cannot_be_resolved_is_reported_and_leaves_the_node_shown() {
    let d = doc(
        json!({"themes": axes(), "variables": {
            "show000001": {"type": "boolean", "name": "flag/show", "values": [{"value": true, "when": {"Mode": "Dark"}}]}
        }}),
        json!([rect("light00001", json!({"visible": var("show000001")}))]),
    );
    let tree = Resolver::new().resolve_page(&d, &d.pages[0]);
    assert!(tree[0].visible, "an unresolved flag does not hide a layer");
    assert_eq!(tree[0].problems.len(), 1);
    assert_eq!(tree[0].problems[0].at, "visible");
    assert!(matches!(
        tree[0].problems[0].error,
        crate::resolver::variables::ResolveError::Variable(VariableError::NoValue { .. })
    ));
}

#[test]
fn wrong_types_are_found_at_every_place_a_variable_can_go() {
    let cases: Vec<(&str, Json, &str, T)> = vec![
        (
            "visible",
            rect("r000000001", json!({"visible": var("n000000001")})),
            "visible",
            T::Number,
        ),
        (
            "opacity",
            rect("r000000001", json!({"opacity": var("c000000001")})),
            "opacity",
            T::Color,
        ),
        (
            "rotation",
            rect("r000000001", json!({"rotation": var("b000000001")})),
            "rotation",
            T::Boolean,
        ),
        (
            "width",
            rect("r000000001", json!({"width": var("c000000001")})),
            "width",
            T::Color,
        ),
        (
            "height",
            rect("r000000001", json!({"height": var("s000000001")})),
            "height",
            T::String,
        ),
        (
            "solid fill",
            rect("r000000001", json!({"fills": solid_with("n000000001")})),
            "fills[0].color",
            T::Number,
        ),
        (
            "gradient stop",
            rect(
                "r000000001",
                json!({"fills": [{"type": "gradient", "kind": "linear",
                "stops": [{"color": "#000000", "position": 0}, {"color": var("n000000001"), "position": 1}]}]}),
            ),
            "fills[0].stops[1].color",
            T::Number,
        ),
        (
            "mesh point",
            rect(
                "r000000001",
                json!({"fills": [{"type": "mesh", "columns": 1, "rows": 1,
                "points": [{"x": 0, "y": 0, "color": var("b000000001")}]}]}),
            ),
            "fills[0].points[0].color",
            T::Boolean,
        ),
        (
            "image opacity",
            rect(
                "r000000001",
                json!({"fills": [{"type": "image", "asset": "asset00001", "opacity": var("c000000001")}]}),
            ),
            "fills[0].opacity",
            T::Color,
        ),
        (
            "stroke width",
            rect(
                "r000000001",
                json!({"stroke": {"paints": [], "width": var("c000000001")}}),
            ),
            "stroke.width",
            T::Color,
        ),
        (
            "stroke side",
            rect(
                "r000000001",
                json!({"stroke": {"paints": [], "width": [1, var("s000000001"), 1, 1]}}),
            ),
            "stroke.width[1]",
            T::String,
        ),
        (
            "stroke paint",
            rect(
                "r000000001",
                json!({"stroke": {"paints": solid_with("n000000001"), "width": 1}}),
            ),
            "stroke.paints[0].color",
            T::Number,
        ),
        (
            "shadow",
            rect(
                "r000000001",
                json!({"effects": [{"type": "drop-shadow", "x": var("c000000001"), "y": 0,
                "blur": 4, "color": "#00000040"}]}),
            ),
            "effects[0].x",
            T::Color,
        ),
        (
            "shadow colour",
            rect(
                "r000000001",
                json!({"effects": [{"type": "inner-shadow", "x": 0, "y": 0, "blur": 4,
                "color": var("n000000001")}]}),
            ),
            "effects[0].color",
            T::Number,
        ),
        (
            "blur",
            rect(
                "r000000001",
                json!({"effects": [{"type": "layer-blur", "radius": var("s000000001")}]}),
            ),
            "effects[0].radius",
            T::String,
        ),
        (
            "shader uniform",
            rect(
                "r000000001",
                json!({"effects": [{"type": "shader", "src": "a.glsl",
                "uniforms": {"tint": var("s000000001")}}]}),
            ),
            "effects[0].uniforms.tint",
            T::String,
        ),
        (
            "frame gap",
            frame("f000000001", json!({"gap": var("c000000001")}), json!([])),
            "gap",
            T::Color,
        ),
        (
            "frame padding side",
            frame(
                "f000000001",
                json!({"padding": [1, 2, var("s000000001"), 4]}),
                json!([]),
            ),
            "padding[2]",
            T::String,
        ),
        (
            "frame padding axis",
            frame(
                "f000000001",
                json!({"padding": [var("b000000001"), 2]}),
                json!([]),
            ),
            "padding[0]",
            T::Boolean,
        ),
        (
            "frame corner",
            frame(
                "f000000001",
                json!({"cornerRadius": [1, 2, var("s000000001"), 4]}),
                json!([]),
            ),
            "cornerRadius[2]",
            T::String,
        ),
        (
            "rect corner",
            rect("r000000001", json!({"cornerRadius": var("c000000001")})),
            "cornerRadius",
            T::Color,
        ),
        (
            "polygon corner",
            json!({"type": "polygon", "id": "p000000001", "name": "p", "width": 10, "height": 10,
                   "sides": 5, "cornerRadius": var("c000000001")}),
            "cornerRadius",
            T::Color,
        ),
        (
            "text run size",
            text(json!({"size": var("c000000001")})),
            "content[0].size",
            T::Color,
        ),
        (
            "text run line height",
            text(json!({"size": 12, "lineHeight": var("s000000001")})),
            "content[0].lineHeight",
            T::String,
        ),
        (
            "text run letter spacing",
            text(json!({"size": 12, "letterSpacing": var("b000000001")})),
            "content[0].letterSpacing",
            T::Boolean,
        ),
        (
            "icon name",
            json!({"type": "icon", "id": "i000000001", "name": "i", "width": 24, "height": 24,
                   "library": "lucide", "icon": var("n000000001"), "weight": 400}),
            "icon",
            T::Number,
        ),
        (
            "note colour",
            json!({"type": "note", "id": "n000000009", "name": "n", "width": 10, "height": 10,
                   "content": "TEST DATA", "color": var("n000000001")}),
            "color",
            T::Number,
        ),
        (
            "embed radius",
            json!({"type": "embed", "id": "e000000001", "name": "e", "width": 10, "height": 10,
                   "url": "https://example.test", "radius": var("c000000001")}),
            "radius",
            T::Color,
        ),
    ];
    for (name, value, path, found) in cases {
        assert_eq!(problems(value), vec![(path.to_owned(), found)], "{name}");
    }
}

#[test]
fn the_right_types_are_accepted_everywhere() {
    let accepted = vec![
        rect(
            "r000000001",
            json!({
                "visible": var("b000000001"), "opacity": var("n000000001"), "rotation": var("n000000001"),
                "width": var("n000000001"), "fills": solid_with("c000000001"),
                "stroke": {"paints": solid_with("c000000001"), "width": [1, var("n000000001"), 1, 1]},
                "effects": [{"type": "drop-shadow", "x": var("n000000001"), "y": 0, "blur": 4, "color": var("c000000001")},
                            {"type": "shader", "src": "a.glsl", "uniforms": {"a": var("c000000001"), "b": var("n000000001"), "c": var("b000000001")}}],
                "cornerRadius": var("n000000001")
            }),
        ),
        frame(
            "f000000001",
            json!({"gap": var("n000000001"), "padding": [var("n000000001"), 2]}),
            json!([]),
        ),
        text(json!({"size": var("n000000001"), "lineHeight": var("n000000001")})),
        json!({"type": "icon", "id": "i000000001", "name": "i", "width": 24, "height": 24,
               "library": "lucide", "icon": var("s000000001"), "weight": 400}),
        json!({"type": "script", "id": "x000000001", "name": "x", "width": 10, "height": 10, "src": "a.js",
               "inputs": {"a": var("n000000001"), "b": var("s000000001"), "c": var("b000000001"),
                          "d": var("c000000001")}}),
        json!({"type": "note", "id": "n000000009", "name": "n", "width": 10, "height": 10,
               "content": "TEST DATA", "color": var("c000000001")}),
    ];
    for value in accepted {
        assert_eq!(problems_any(value.clone()), Vec::<String>::new(), "{value}");
    }
}

#[test]
fn a_reference_to_a_missing_variable_is_a_binding_problem_too() {
    let found = check_node_bindings(
        &bound(),
        &node(rect("r000000001", json!({"opacity": var("gone000001")}))),
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].at, "opacity");
    assert_eq!(found[0].variable.as_str(), "gone000001");
    assert_eq!(found[0].found(), None);
}

#[test]
fn text_and_effect_styles_are_checked_like_nodes() {
    let d = bound();
    let style: vellum_types::Style = serde_json::from_value(json!({
        "type": "text", "name": "Body", "font": "Inter", "size": var("c000000001"),
        "letterSpacing": var("n000000001")
    }))
    .expect("a style");
    let found = check_style_bindings(&d, &style);
    assert_eq!(
        found.iter().map(|p| p.at.as_str()).collect::<Vec<_>>(),
        vec!["size"]
    );
    let effect: vellum_types::Style = serde_json::from_value(json!({
        "type": "effect", "name": "Soft", "effects": [{"type": "layer-blur", "radius": var("c000000001")}]
    })).expect("a style");
    let found = check_style_bindings(&d, &effect);
    assert_eq!(
        found.iter().map(|p| p.at.as_str()).collect::<Vec<_>>(),
        vec!["effects[0].radius"]
    );
}

fn solid_with(variable: &str) -> Json {
    json!([{"type": "solid", "color": var(variable)}])
}

fn text(run: Json) -> Json {
    let mut run = run;
    run["text"] = json!("TEST DATA");
    run["font"] = json!("Inter");
    json!({"type": "text", "id": "t000000001", "name": "t", "width": "hug", "height": "hug", "content": [run]})
}

/// The paths of every problem, whatever kind.
fn problems_any(value: Json) -> Vec<String> {
    check_node_bindings(&bound(), &node(value))
        .into_iter()
        .map(|p| p.at)
        .collect()
}

// ---- through the op layer ----

fn doc_for_ops() -> vellum_types::Document {
    c::document()
}

#[track_caller]
fn expect_binding(ops: Vec<Op>, path: &str, variable: &str, expected: &'static [T], found: T) {
    let error = match crate::ops::plan(&doc_for_ops(), &c::tx(ops), &mut c::Ids::default()) {
        Ok(_) => panic!("the transaction should have been refused"),
        Err(error) => error,
    };
    assert_eq!(
        (error.path.as_str(), &error.kind),
        (
            path,
            &OpErrorKind::BindingType {
                variable: c::var_id(variable),
                expected,
                found
            }
        ),
        "{error}"
    );
}

fn update(id: &str, patch: Json) -> Op {
    Op::Update {
        id: c::id(id),
        patch: c::patch(patch),
    }
}

#[test]
fn an_update_that_binds_the_wrong_type_is_refused_with_the_property_path() {
    expect_binding(
        vec![update("rect000003", json!({"opacity": var("color00001")}))],
        "patch.opacity",
        "color00001",
        &[T::Number],
        T::Color,
    );
    expect_binding(
        vec![update(
            "rect000003",
            json!({"fills": solid_with("number0001")}),
        )],
        "patch.fills[0].color",
        "number0001",
        &[T::Color],
        T::Number,
    );
    expect_binding(
        vec![update("rect000003", json!({"visible": var("number0001")}))],
        "patch.visible",
        "number0001",
        &[T::Boolean],
        T::Number,
    );
}

#[test]
fn the_right_type_is_accepted_by_an_update() {
    let d = c::accepted(vec![update(
        "rect000003",
        json!({"opacity": var("number0001")}),
    )]);
    assert!(check_document(&d).is_empty());
}

#[test]
fn an_inserted_subtree_is_checked_all_the_way_down() {
    let nested = c::node(frame(
        "fnew000001",
        json!({}),
        json!([rect(
            "rnew000001",
            json!({"fills": solid_with("number0001")})
        )]),
    ));
    expect_binding(
        vec![Op::Insert {
            to: c::on_page("canvas0001", 0),
            node: Box::new(nested),
        }],
        "node.children[0].fills[0].color",
        "number0001",
        &[T::Color],
        T::Number,
    );
}

#[test]
fn replace_page_component_and_style_inputs_are_checked() {
    let bad = c::node(rect("rnew000001", json!({"opacity": var("color00001")})));
    expect_binding(
        vec![Op::Replace {
            id: c::id("rect000003"),
            node: Box::new(bad.clone()),
        }],
        "node.opacity",
        "color00001",
        &[T::Number],
        T::Color,
    );
    let page = vellum_types::Page {
        id: c::page_id("newpage001"),
        name: "TEST DATA".into(),
        kind: vellum_types::PageKind::Canvas,
        children: vec![bad],
    };
    expect_binding(
        vec![Op::InsertPage {
            index: 0,
            page: Box::new(page),
        }],
        "page.children[0].opacity",
        "color00001",
        &[T::Number],
        T::Color,
    );
}

#[test]
fn changing_the_type_of_a_variable_in_use_is_refused_at_the_variable() {
    // `color00001` is the fill of `text000001`.
    expect_binding(
        vec![Op::SetVariables {
            set: BTreeMap::from([(
                c::var_id("color00001"),
                c::variable(
                    json!({"type": "number", "name": "TEST DATA now a number", "values": [{"value": 1}]}),
                ),
            )]),
            remove: BTreeSet::new(),
        }],
        "set.color00001",
        "color00001",
        &[T::Color],
        T::Number,
    );
}

#[test]
fn changing_the_type_of_a_variable_nothing_uses_is_allowed() {
    let d = c::accepted(vec![Op::SetVariables {
        set: BTreeMap::from([(
            c::var_id("number0001"),
            c::variable(
                json!({"type": "string", "name": "TEST DATA now a string", "values": [{"value": "a"}]}),
            ),
        )]),
        remove: BTreeSet::new(),
    }]);
    assert!(check_document(&d).is_empty());
}

#[test]
fn the_whole_document_check_reports_every_mismatch() {
    let mut d = doc_for_ops();
    // Built around the op layer, as a merge or a hand-edited file could.
    let patched = c::find(&d, "rect000003")
        .expect("a rect")
        .patched(&c::patch(json!({"opacity": var("color00001")})))
        .expect("a patch");
    d.pages[0].children[2] = patched;
    let problems = check_document(&d);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("rect000003") && p.contains("opacity") && p.contains("color00001")),
        "{problems:?}"
    );
    let _ = ALICE;
}
