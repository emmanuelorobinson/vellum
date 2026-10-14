//! Theme axes: the first value is the default, a frame's theme reaches its subtree, and a child that sets
//! the same axis wins (ARCHITECTURE §4.1, DESIGN.md C34).

use std::collections::BTreeMap;

use serde_json::json;

use super::{axes, doc, frame, rect, theme};
use crate::resolver::variables::{ResolveError, Resolver, Theme, ThemeError};

fn choices(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(axis, value)| ((*axis).into(), (*value).into()))
        .collect()
}

#[test]
fn the_first_value_of_each_axis_is_the_default() {
    let d = doc(json!({"themes": axes()}), json!([]));
    let t = Theme::defaults(&d);
    assert_eq!(t.get("Mode"), Some("Light"));
    assert_eq!(t.get("Device"), Some("Phone"));
    assert_eq!(t.get("Brand"), None);
}

#[test]
fn an_axis_with_no_values_has_no_default() {
    let d = doc(json!({"themes": {"Brand": []}}), json!([]));
    assert_eq!(Theme::defaults(&d).get("Brand"), None);
}

#[test]
fn a_document_without_axes_has_an_empty_theme() {
    let d = doc(json!({}), json!([]));
    assert_eq!(Theme::defaults(&d), Theme::default());
}

#[test]
fn a_node_theme_overrides_the_inherited_one_for_its_subtree() {
    let d = doc(
        json!({"themes": axes()}),
        json!([
            frame(
                "outer00001",
                json!({"theme": {"Mode": "Dark"}}),
                json!([frame(
                    "inner00001",
                    json!({}),
                    json!([rect("leaf000001", json!({}))])
                )])
            ),
            rect("after00001", json!({}))
        ]),
    );
    let tree = Resolver::new().resolve_page(&d, &d.pages[0]);
    let outer = &tree[0];
    let inner = &outer.children[0];
    let leaf = &inner.children[0];
    for resolved in [outer, inner, leaf] {
        assert_eq!(resolved.theme.get("Mode"), Some("Dark"));
        assert_eq!(
            resolved.theme.get("Device"),
            Some("Phone"),
            "unset axes stay at their default"
        );
    }
    assert_eq!(
        tree[1].theme.get("Mode"),
        Some("Light"),
        "a sibling of the frame is untouched"
    );
}

#[test]
fn a_child_that_sets_the_same_axis_wins_for_its_own_subtree() {
    let d = doc(
        json!({"themes": axes()}),
        json!([frame(
            "outer00001",
            json!({"theme": {"Mode": "Dark"}}),
            json!([
                frame(
                    "inner00001",
                    json!({"theme": {"Mode": "Light"}}),
                    json!([rect("deep000001", json!({}))])
                ),
                rect("other00001", json!({}))
            ])
        )]),
    );
    let tree = Resolver::new().resolve_page(&d, &d.pages[0]);
    let outer = &tree[0];
    assert_eq!(outer.children[0].theme.get("Mode"), Some("Light"));
    assert_eq!(
        outer.children[0].children[0].theme.get("Mode"),
        Some("Light")
    );
    assert_eq!(outer.children[1].theme.get("Mode"), Some("Dark"));
}

#[test]
fn a_child_that_sets_another_axis_keeps_the_inherited_one() {
    let d = doc(
        json!({"themes": axes()}),
        json!([frame(
            "outer00001",
            json!({"theme": {"Mode": "Dark"}}),
            json!([frame(
                "inner00001",
                json!({"theme": {"Device": "Tablet"}}),
                json!([])
            )])
        )]),
    );
    let tree = Resolver::new().resolve_page(&d, &d.pages[0]);
    let inner = &tree[0].children[0];
    assert_eq!(inner.theme.get("Mode"), Some("Dark"));
    assert_eq!(inner.theme.get("Device"), Some("Tablet"));
}

#[test]
fn a_theme_is_applied_per_axis_and_replaces_nothing_else() {
    let d = doc(json!({"themes": axes()}), json!([]));
    let base = theme(&d, &[("Mode", "Dark")]);
    let next = base
        .with(&d, &choices(&[("Device", "Tablet")]))
        .expect("valid");
    assert_eq!(next.get("Mode"), Some("Dark"));
    assert_eq!(next.get("Device"), Some("Tablet"));
    assert_eq!(
        base.get("Device"),
        Some("Phone"),
        "the inherited theme is not changed"
    );
}

#[test]
fn an_unknown_axis_or_value_is_a_typed_error() {
    let d = doc(json!({"themes": axes()}), json!([]));
    let base = Theme::defaults(&d);
    assert_eq!(
        base.with(&d, &choices(&[("Brand", "A")])),
        Err(ThemeError::UnknownAxis {
            axis: "Brand".into()
        })
    );
    assert_eq!(
        base.with(&d, &choices(&[("Mode", "Sepia")])),
        Err(ThemeError::UnknownValue {
            axis: "Mode".into(),
            value: "Sepia".into()
        })
    );
}

#[test]
fn a_bad_theme_on_a_frame_is_reported_and_the_rest_of_it_still_applies() {
    let d = doc(
        json!({"themes": axes()}),
        json!([frame(
            "outer00001",
            json!({"theme": {"Mode": "Sepia", "Device": "Tablet"}}),
            json!([])
        )]),
    );
    let tree = Resolver::new().resolve_page(&d, &d.pages[0]);
    let outer = &tree[0];
    assert_eq!(
        outer.theme.get("Mode"),
        Some("Light"),
        "the bad entry is ignored"
    );
    assert_eq!(outer.theme.get("Device"), Some("Tablet"));
    assert_eq!(outer.problems.len(), 1);
    assert_eq!(outer.problems[0].at, "theme.Mode");
    assert_eq!(
        outer.problems[0].error,
        ResolveError::Theme(ThemeError::UnknownValue {
            axis: "Mode".into(),
            value: "Sepia".into()
        })
    );
}
