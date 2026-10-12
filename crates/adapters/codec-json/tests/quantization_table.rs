//! Rounding is decided by the name of the field, so a field added to the types without a decision here would
//! be written unrounded. This test reads the JSON Schema and fails until every number the format defines has
//! a unit or is listed as one the format gives no unit to.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use vellum_codec_json::{EXACT_NUMBER_KEYS, quantum_of};
use vellum_types::Document;

/// Whether a schema can be a bare number, or an array of numbers (through references and alternatives, but
/// not through an object: its properties are checked on their own).
fn holds_number(schema: &Json, defs: &Json, depth: usize) -> bool {
    if depth > 12 {
        return false;
    }
    if let Some(target) = schema.get("$ref").and_then(Json::as_str) {
        let name = target.rsplit('/').next().expect("a name");
        return defs
            .get(name)
            .is_some_and(|d| holds_number(d, defs, depth + 1));
    }
    let is_number = match schema.get("type") {
        Some(Json::String(t)) => t == "number",
        Some(Json::Array(ts)) => ts.iter().any(|t| t == "number"),
        _ => false,
    };
    if is_number {
        return true;
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(Json::Array(options)) = schema.get(key)
            && options.iter().any(|o| holds_number(o, defs, depth + 1))
        {
            return true;
        }
    }
    match schema.get("items") {
        Some(items) => holds_number(items, defs, depth + 1),
        None => false,
    }
}

fn number_properties(schema: &Json, defs: &Json, out: &mut BTreeSet<String>) {
    match schema {
        Json::Object(map) => {
            if let Some(Json::Object(properties)) = map.get("properties") {
                for (key, property) in properties {
                    if holds_number(property, defs, 0) {
                        out.insert(key.clone());
                    }
                }
            }
            map.values().for_each(|v| number_properties(v, defs, out));
        }
        Json::Array(items) => items.iter().for_each(|v| number_properties(v, defs, out)),
        _ => {}
    }
}

fn schema_keys() -> BTreeSet<String> {
    let schema = serde_json::to_value(schemars::schema_for!(Document)).expect("schema");
    let defs = schema.get("$defs").cloned().unwrap_or(Json::Null);
    let mut keys = BTreeSet::new();
    number_properties(&schema, &defs, &mut keys);
    keys
}

#[test]
fn the_search_finds_the_numbers_it_should() {
    let keys = schema_keys();
    for expected in [
        "opacity",
        "width",
        "hug",
        "dash",
        "transform",
        "position",
        "rotation",
    ] {
        assert!(
            keys.contains(expected),
            "{expected} is not found in {keys:?}"
        );
    }
    for not_a_number in ["name", "id", "columns", "sides", "weight", "durationMs"] {
        assert!(
            !keys.contains(not_a_number),
            "{not_a_number} is an integer or text"
        );
    }
}

#[test]
fn every_number_the_format_defines_has_a_unit_or_is_listed_as_having_none() {
    for key in schema_keys() {
        assert!(
            quantum_of(&key).is_some() || EXACT_NUMBER_KEYS.contains(&key.as_str()),
            "the schema has a number called {key}; decide how the codec rounds it"
        );
    }
}

#[test]
fn the_table_names_no_field_the_format_lacks() {
    let keys = schema_keys();
    for key in EXACT_NUMBER_KEYS {
        assert!(
            keys.contains(*key),
            "{key} is listed as exact but is not a number in the schema"
        );
        assert!(quantum_of(key).is_none(), "{key} is both exact and rounded");
    }
    for key in [
        "width",
        "height",
        "x",
        "y",
        "gap",
        "padding",
        "cornerRadius",
        "radius",
        "blur",
        "spread",
        "size",
        "lineHeight",
        "letterSpacing",
        "dash",
        "hug",
        "fill",
        "opacity",
        "position",
        "innerRadius",
        "innerRatio",
        "zoom",
        "crop",
        "transform",
        "rotation",
        "startAngle",
        "sweepAngle",
    ] {
        assert!(quantum_of(key).is_some(), "{key} lost its unit");
        assert!(
            keys.contains(key),
            "{key} has a unit but is not a number in the schema"
        );
    }
}
