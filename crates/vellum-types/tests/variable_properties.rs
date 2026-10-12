//! Property tests for variables (guide Prompt 8). Any variable the types can hold prints to JSON, parses back to
//! an equal variable, and prints the same bytes again. A literal and an alias differ only by their key, so the
//! generators cover both, with and without theme conditions, for every kind of variable.

use std::collections::BTreeMap;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use vellum_types::{
    AliasEntry, Color, LiteralEntry, Variable, VariableBody, VariableId, VariableValue,
};

fn arb_id() -> impl Strategy<Value = VariableId> {
    "[A-Za-z0-9_-]{1,24}".prop_map(|text| VariableId::new(text).expect("valid id"))
}

fn arb_when() -> impl Strategy<Value = BTreeMap<String, String>> {
    btree_map("[A-Za-z]{1,6}", "[a-z]{1,6}", 0..3)
}

fn arb_entry<T: std::fmt::Debug + 'static>(
    literal: impl Strategy<Value = T> + 'static,
) -> impl Strategy<Value = VariableValue<T>> {
    prop_oneof![
        (literal, arb_when())
            .prop_map(|(value, when)| VariableValue::Literal(LiteralEntry { value, when })),
        (arb_id(), arb_when())
            .prop_map(|(alias, when)| VariableValue::Alias(AliasEntry { alias, when })),
    ]
}

fn arb_variable() -> impl Strategy<Value = Variable> {
    let name = "[a-z]{1,6}(/[a-z]{1,6}){0,2}";
    let color = (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Color::rgba(r, g, b, a));
    prop_oneof![
        (name, vec(arb_entry(color), 0..4))
            .prop_map(|(name, values)| Variable::Color(VariableBody { name, values })),
        (name, vec(arb_entry(-1.0e6..1.0e6_f64), 0..4))
            .prop_map(|(name, values)| { Variable::Number(VariableBody { name, values }) }),
        // Any text, including text that looks like a colour: a string variable keeps it as written.
        (name, vec(arb_entry("\\PC{0,12}"), 0..4))
            .prop_map(|(name, values)| { Variable::String(VariableBody { name, values }) }),
        (name, vec(arb_entry(any::<bool>()), 0..4))
            .prop_map(|(name, values)| { Variable::Boolean(VariableBody { name, values }) }),
    ]
}

proptest! {
    #[test]
    fn a_variable_survives_a_trip_through_json(variable in arb_variable()) {
        let text = serde_json::to_string(&variable).expect("serialise");
        let back: Variable = serde_json::from_str(&text).expect("parse");
        prop_assert_eq!(&back, &variable);
        prop_assert_eq!(serde_json::to_string(&back).expect("serialise"), text);
    }

    #[test]
    fn a_variable_names_its_own_type(variable in arb_variable()) {
        let json = serde_json::to_value(&variable).expect("serialise");
        prop_assert_eq!(json["type"].as_str(), Some(variable.variable_type().as_str()));
    }
}
