//! Variables (ARCHITECTURE §4.1). A variable has one type and ordered values. A value is a literal of that
//! type or an alias to another variable of the same type, and it may apply only under some theme axis
//! values. Choosing the value that applies (last match wins) is resolution, Prompt 16. Here the file only
//! has to say exactly what each entry is.

use serde_json::{Value as Json, json};
use vellum_types::{Value, VarRef, Variable, VariableId, VariableType};

fn outcome(input: Json) -> Result<Variable, serde_json::Error> {
    serde_json::from_value(input)
}

fn canonical(input: Json) -> Variable {
    let variable = outcome(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"));
    let back = serde_json::to_value(&variable).expect("serialise");
    assert_eq!(
        back, input,
        "the variable did not print back as it was read"
    );
    variable
}

#[test]
fn a_variable_has_a_type_a_name_and_ordered_values() {
    let variable = canonical(json!({
        "type": "color",
        "name": "color/primary",
        "values": [
            {"value": "#1A2B3C"},
            {"value": "#E0E8F0", "when": {"Mode": "dark"}}
        ]
    }));
    assert_eq!(variable.variable_type(), VariableType::Color);
    assert_eq!(variable.name(), "color/primary");
}

#[test]
fn a_colour_is_read_in_either_case_and_printed_in_upper_case() {
    let variable = outcome(json!({
        "type": "color",
        "name": "color/ink",
        "values": [{"value": "#1a2b3c"}]
    }))
    .expect("a colour in lower case");
    assert_eq!(
        serde_json::to_value(&variable).expect("serialise")["values"][0]["value"],
        "#1A2B3C"
    );
    assert!(
        outcome(json!({"type": "color", "name": "c", "values": [{"value": "#FFF"}]})).is_err(),
        "a three-digit colour is not a colour"
    );
}

#[test]
fn a_string_variable_keeps_its_text_exactly_as_written() {
    // A string that looks like a colour stays a string: the variable's type decides how its literals read.
    canonical(json!({
        "type": "string",
        "name": "font/accent",
        "values": [{"value": "#ff0000"}]
    }));
}

#[test]
fn a_number_variable_takes_numbers_and_a_boolean_takes_true_or_false() {
    canonical(json!({"type": "number", "name": "space/md", "values": [{"value": 12.0}]}));
    canonical(json!({"type": "boolean", "name": "show/badge", "values": [{"value": true}]}));
    assert!(
        outcome(json!({"type": "number", "name": "n", "values": [{"value": "12"}]})).is_err(),
        "a number variable takes no text"
    );
    assert!(
        outcome(json!({"type": "boolean", "name": "b", "values": [{"value": 1.0}]})).is_err(),
        "a boolean variable takes no number"
    );
}

#[test]
fn an_entry_holds_a_literal_or_an_alias_and_never_both_or_neither() {
    assert!(
        outcome(json!({
            "type": "color",
            "name": "c",
            "values": [{"value": "#FFFFFF", "$alias": "va000001"}]
        }))
        .is_err()
    );
    assert!(outcome(json!({"type": "color", "name": "c", "values": [{}]})).is_err());
    assert!(
        outcome(json!({
            "type": "color",
            "name": "c",
            "values": [{"value": "#FFFFFF", "extra": 1}]
        }))
        .is_err()
    );
}

#[test]
fn an_alias_names_another_variable_by_id_and_keeps_its_condition() {
    canonical(json!({
        "type": "color",
        "name": "color/accent",
        "values": [
            {"$alias": "va000001"},
            {"$alias": "va000002", "when": {"Mode": "dark"}}
        ]
    }));
}

#[test]
fn an_alias_cannot_name_a_variable_by_its_path() {
    assert!(
        outcome(json!({
            "type": "color",
            "name": "c",
            "values": [{"$alias": "color/primary"}]
        }))
        .is_err()
    );
}

#[test]
fn a_variable_takes_its_four_types_and_no_other_fields() {
    let words: Vec<&str> = VariableType::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(words, ["color", "number", "string", "boolean"]);
    let message = outcome(json!({"type": "gradient", "name": "g", "values": []}))
        .expect_err("gradient is not a variable type")
        .to_string();
    assert!(message.contains("gradient"), "{message}");
    assert!(outcome(json!({"type": "number", "name": "n", "values": [], "extra": 1})).is_err());
    assert!(
        outcome(json!({"type": "number", "values": []})).is_err(),
        "a variable has a name"
    );
}

#[test]
fn a_reference_names_a_variable_by_its_id() {
    let value: Value<f64> =
        serde_json::from_value(json!({"$var": "va000001"})).expect("an id is a reference");
    assert_eq!(
        value,
        Value::Var(VarRef::new("va000001").expect("a valid id"))
    );
    assert!(
        serde_json::from_value::<Value<f64>>(json!({"$var": "color/primary"})).is_err(),
        "a path is not an id"
    );
}

#[test]
fn a_reference_is_made_from_a_variable_id_and_gives_it_back() {
    let id = VariableId::new("va000001").expect("a valid id");
    let reference = VarRef::to(id.clone());
    assert_eq!(reference.id(), &id);
    assert_eq!(
        serde_json::to_value(&reference).expect("serialise"),
        json!({"$var": "va000001"})
    );
}
