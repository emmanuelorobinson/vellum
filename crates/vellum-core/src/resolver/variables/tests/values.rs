//! A variable's value under a theme: the last matching entry wins, aliases follow the chain under the same
//! theme, and a chain that comes back to itself is a typed error.

use serde_json::json;
use vellum_types::{Color, Scalar, Uniform, Value, VarRef, VariableId, VariableType};

use super::{axes, doc, theme};
use crate::resolver::variables::{ResolvedValue, VariableError, resolve_value, resolve_variable};

fn id(text: &str) -> VariableId {
    VariableId::new(text).expect("a valid id")
}

fn var<T>(text: &str) -> Value<T> {
    Value::Var(VarRef::new(text).expect("a valid id"))
}

fn doc_with(variables: serde_json::Value) -> vellum_types::Document {
    doc(json!({"themes": axes(), "variables": variables}), json!([]))
}

fn surface() -> vellum_types::Document {
    doc_with(json!({
        "surface0001": {"type": "color", "name": "color/surface", "values": [
            {"value": "#FFFFFF"},
            {"value": "#111111", "when": {"Mode": "Dark"}},
            {"value": "#EEEEEE", "when": {"Device": "Tablet"}}
        ]}
    }))
}

fn color(text: &str) -> ResolvedValue {
    ResolvedValue::Color(text.parse::<Color>().expect("a colour"))
}

#[test]
fn the_last_matching_entry_wins() {
    let d = surface();
    let at =
        |choices: &[(&str, &str)]| resolve_variable(&d, &id("surface0001"), &theme(&d, choices));
    assert_eq!(at(&[]), Ok(color("#FFFFFF")));
    assert_eq!(at(&[("Mode", "Dark")]), Ok(color("#111111")));
    assert_eq!(at(&[("Device", "Tablet")]), Ok(color("#EEEEEE")));
    // Both the Dark entry and the Tablet entry match; the Tablet one is later.
    assert_eq!(
        at(&[("Mode", "Dark"), ("Device", "Tablet")]),
        Ok(color("#EEEEEE"))
    );
}

#[test]
fn entry_order_decides_not_how_specific_an_entry_is() {
    let d = doc_with(json!({
        "surface0001": {"type": "color", "name": "color/surface", "values": [
            {"value": "#EEEEEE", "when": {"Device": "Tablet"}},
            {"value": "#111111", "when": {"Mode": "Dark"}}
        ]}
    }));
    let both = theme(&d, &[("Mode", "Dark"), ("Device", "Tablet")]);
    assert_eq!(
        resolve_variable(&d, &id("surface0001"), &both),
        Ok(color("#111111"))
    );
}

#[test]
fn an_entry_needs_every_axis_it_names() {
    let d = doc_with(json!({
        "pair0000001": {"type": "number", "name": "space/pair", "values": [
            {"value": 1},
            {"value": 2, "when": {"Mode": "Dark", "Device": "Tablet"}}
        ]}
    }));
    let at =
        |choices: &[(&str, &str)]| resolve_variable(&d, &id("pair0000001"), &theme(&d, choices));
    assert_eq!(at(&[("Mode", "Dark")]), Ok(ResolvedValue::Number(1.0)));
    assert_eq!(at(&[("Device", "Tablet")]), Ok(ResolvedValue::Number(1.0)));
    assert_eq!(
        at(&[("Mode", "Dark"), ("Device", "Tablet")]),
        Ok(ResolvedValue::Number(2.0))
    );
}

#[test]
fn an_entry_that_names_an_axis_the_document_lacks_never_matches() {
    let d = doc_with(json!({
        "brand000001": {"type": "string", "name": "text/brand", "values": [
            {"value": "plain"},
            {"value": "branded", "when": {"Brand": "A"}}
        ]}
    }));
    assert_eq!(
        resolve_variable(&d, &id("brand000001"), &theme(&d, &[])),
        Ok(ResolvedValue::String("plain".into()))
    );
}

#[test]
fn no_matching_entry_is_a_typed_error() {
    let d = doc_with(json!({
        "dark0000001": {"type": "boolean", "name": "flag/dark", "values": [{"value": true, "when": {"Mode": "Dark"}}]},
        "empty000001": {"type": "number", "name": "space/none", "values": []}
    }));
    let light = theme(&d, &[]);
    assert!(matches!(
        resolve_variable(&d, &id("dark0000001"), &light),
        Err(VariableError::NoValue { variable, .. }) if variable == id("dark0000001")
    ));
    assert!(matches!(
        resolve_variable(&d, &id("empty000001"), &light),
        Err(VariableError::NoValue { .. })
    ));
}

#[test]
fn an_unknown_variable_is_a_typed_error() {
    let d = doc_with(json!({}));
    assert_eq!(
        resolve_variable(&d, &id("missing0001"), &theme(&d, &[])),
        Err(VariableError::Unknown(id("missing0001")))
    );
}

#[test]
fn every_type_resolves_to_its_own_kind_of_value() {
    let d = doc_with(json!({
        "c000000001": {"type": "color", "name": "c", "values": [{"value": "#FF000080"}]},
        "n000000001": {"type": "number", "name": "n", "values": [{"value": 12.5}]},
        "s000000001": {"type": "string", "name": "s", "values": [{"value": "#ff0000"}]},
        "b000000001": {"type": "boolean", "name": "b", "values": [{"value": true}]}
    }));
    let t = theme(&d, &[]);
    assert_eq!(
        resolve_variable(&d, &id("c000000001"), &t),
        Ok(color("#FF000080"))
    );
    assert_eq!(
        resolve_variable(&d, &id("n000000001"), &t),
        Ok(ResolvedValue::Number(12.5))
    );
    assert_eq!(
        resolve_variable(&d, &id("s000000001"), &t),
        Ok(ResolvedValue::String("#ff0000".into())),
        "a string variable keeps colour-looking text as the text it is"
    );
    assert_eq!(
        resolve_variable(&d, &id("b000000001"), &t),
        Ok(ResolvedValue::Boolean(true))
    );
}

fn chain() -> vellum_types::Document {
    doc_with(json!({
        "base000001": {"type": "number", "name": "space/base", "values": [
            {"value": 4}, {"value": 8, "when": {"Device": "Tablet"}}
        ]},
        "mid0000001": {"type": "number", "name": "space/mid", "values": [{"$alias": "base000001"}]},
        "top0000001": {"type": "number", "name": "space/top", "values": [
            {"value": 1}, {"$alias": "mid0000001", "when": {"Mode": "Dark"}}
        ]}
    }))
}

#[test]
fn an_alias_chain_resolves_to_the_end_of_the_chain() {
    let d = chain();
    let at =
        |choices: &[(&str, &str)]| resolve_variable(&d, &id("top0000001"), &theme(&d, choices));
    assert_eq!(
        at(&[]),
        Ok(ResolvedValue::Number(1.0)),
        "the alias entry does not apply in Light"
    );
    assert_eq!(at(&[("Mode", "Dark")]), Ok(ResolvedValue::Number(4.0)));
}

#[test]
fn an_alias_target_picks_its_own_entry_under_the_same_theme() {
    let d = chain();
    let t = theme(&d, &[("Mode", "Dark"), ("Device", "Tablet")]);
    assert_eq!(
        resolve_variable(&d, &id("top0000001"), &t),
        Ok(ResolvedValue::Number(8.0))
    );
}

#[test]
fn an_alias_to_a_missing_variable_is_a_typed_error() {
    let d = doc_with(json!({
        "a000000001": {"type": "number", "name": "a", "values": [{"$alias": "gone000001"}]}
    }));
    assert_eq!(
        resolve_variable(&d, &id("a000000001"), &theme(&d, &[])),
        Err(VariableError::Unknown(id("gone000001")))
    );
}

#[test]
fn an_alias_to_a_variable_of_another_type_is_a_typed_error() {
    let d = doc_with(json!({
        "a000000001": {"type": "color", "name": "a", "values": [{"$alias": "n000000001"}]},
        "n000000001": {"type": "number", "name": "n", "values": [{"value": 1}]}
    }));
    assert_eq!(
        resolve_variable(&d, &id("a000000001"), &theme(&d, &[])),
        Err(VariableError::AliasType {
            variable: id("a000000001"),
            alias: id("n000000001"),
            expected: VariableType::Color,
            found: VariableType::Number,
        })
    );
}

#[test]
fn aliases_that_come_back_are_a_cycle_naming_the_chain() {
    let d = doc_with(json!({
        "a000000001": {"type": "number", "name": "a", "values": [{"$alias": "b000000001"}]},
        "b000000001": {"type": "number", "name": "b", "values": [{"$alias": "c000000001"}]},
        "c000000001": {"type": "number", "name": "c", "values": [{"$alias": "b000000001"}]}
    }));
    // Entered from outside the loop: a leads to a loop it is not in, so the loop is b, c, b.
    assert_eq!(
        resolve_variable(&d, &id("a000000001"), &theme(&d, &[])),
        Err(VariableError::Cycle(vec![
            id("b000000001"),
            id("c000000001"),
            id("b000000001")
        ]))
    );
}

#[test]
fn a_variable_that_aliases_itself_is_a_cycle() {
    let d = doc_with(json!({
        "a000000001": {"type": "number", "name": "a", "values": [{"$alias": "a000000001"}]}
    }));
    assert_eq!(
        resolve_variable(&d, &id("a000000001"), &theme(&d, &[])),
        Err(VariableError::Cycle(vec![
            id("a000000001"),
            id("a000000001")
        ]))
    );
}

#[test]
fn a_cycle_only_counts_when_the_theme_selects_it() {
    let d = doc_with(json!({
        "c000000001": {"type": "number", "name": "c", "values": [
            {"value": 1}, {"$alias": "d000000001", "when": {"Mode": "Dark"}}
        ]},
        "d000000001": {"type": "number", "name": "d", "values": [{"$alias": "c000000001"}]}
    }));
    assert_eq!(
        resolve_variable(&d, &id("c000000001"), &theme(&d, &[])),
        Ok(ResolvedValue::Number(1.0))
    );
    assert_eq!(
        resolve_variable(&d, &id("c000000001"), &theme(&d, &[("Mode", "Dark")])),
        Err(VariableError::Cycle(vec![
            id("c000000001"),
            id("d000000001"),
            id("c000000001")
        ]))
    );
}

#[test]
fn a_literal_resolves_to_itself_and_a_reference_to_its_variable() {
    let d = chain();
    let t = theme(&d, &[("Mode", "Dark")]);
    assert_eq!(resolve_value(&d, &Value::Literal(3.0_f64), &t), Ok(3.0));
    assert_eq!(resolve_value(&d, &var::<f64>("top0000001"), &t), Ok(4.0));
}

#[test]
fn a_reference_to_a_variable_of_the_wrong_type_is_a_typed_error() {
    let d = chain();
    let t = theme(&d, &[]);
    assert_eq!(
        resolve_value::<Color>(&d, &var("top0000001"), &t),
        Err(VariableError::TypeMismatch {
            variable: id("top0000001"),
            expected: &[VariableType::Color],
            found: VariableType::Number,
        })
    );
    assert_eq!(
        resolve_value::<bool>(&d, &var("top0000001"), &t),
        Err(VariableError::TypeMismatch {
            variable: id("top0000001"),
            expected: &[VariableType::Boolean],
            found: VariableType::Number,
        })
    );
}

#[test]
fn script_inputs_and_shader_uniforms_take_the_types_they_allow() {
    let d = doc_with(json!({
        "c000000001": {"type": "color", "name": "c", "values": [{"value": "#112233"}]},
        "n000000001": {"type": "number", "name": "n", "values": [{"value": 2}]},
        "s000000001": {"type": "string", "name": "s", "values": [{"value": "x"}]},
        "b000000001": {"type": "boolean", "name": "b", "values": [{"value": false}]}
    }));
    let t = theme(&d, &[]);
    assert_eq!(
        resolve_value::<Scalar>(&d, &var("n000000001"), &t),
        Ok(Scalar::Number(2.0))
    );
    assert_eq!(
        resolve_value::<Scalar>(&d, &var("s000000001"), &t),
        Ok(Scalar::Text("x".into()))
    );
    assert_eq!(
        resolve_value::<Scalar>(&d, &var("b000000001"), &t),
        Ok(Scalar::Bool(false))
    );
    assert_eq!(
        resolve_value::<Scalar>(&d, &var("c000000001"), &t),
        Ok(Scalar::Text("#112233".into())),
        "a script reads a colour as the text a literal colour is written as"
    );
    assert_eq!(
        resolve_value::<Uniform>(&d, &var("c000000001"), &t),
        Ok(Uniform::Color("#112233".parse().expect("a colour")))
    );
    assert_eq!(
        resolve_value::<Uniform>(&d, &var("n000000001"), &t),
        Ok(Uniform::Number(2.0))
    );
    assert!(matches!(
        resolve_value::<Uniform>(&d, &var("s000000001"), &t),
        Err(VariableError::TypeMismatch { .. })
    ));
}

#[test]
fn a_mismatch_is_reported_before_the_variable_is_resolved() {
    // The colour variable has no value in Light; asking for it as a number is still the first thing wrong.
    let d = doc_with(json!({
        "c000000001": {"type": "color", "name": "c", "values": [{"value": "#112233", "when": {"Mode": "Dark"}}]}
    }));
    assert!(matches!(
        resolve_value::<f64>(&d, &var("c000000001"), &theme(&d, &[])),
        Err(VariableError::TypeMismatch { .. })
    ));
}
