//! A spec format names itself, is stable, refuses a layer that is not there, and lists every token it refers to.
use std::collections::BTreeSet;

use serde_json::Value;
use vellum_core::ports::SpecFormat;
use vellum_types::{Document, NodeId};

fn references(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(name)) = map.get("$token") {
                found.insert(name.clone());
            }
            map.values().for_each(|v| references(v, found));
        }
        Value::Array(items) => items.iter().for_each(|v| references(v, found)),
        _ => {}
    }
}

/// `frame` is a layer of `doc`.
pub fn format(format: &dyn SpecFormat, doc: &Document, frame: &NodeId) {
    let spec = format
        .build(doc, frame)
        .expect("SpecFormat: build of a layer that exists failed");
    assert_eq!(
        spec.get("format").and_then(Value::as_str),
        Some(format.id()),
        "SpecFormat: the description does not name its format"
    );
    assert_eq!(
        spec,
        format.build(doc, frame).expect("SpecFormat: build failed"),
        "SpecFormat: the same layer gave different descriptions"
    );
    let missing = NodeId::new("__contract_missing_layer__").expect("a valid id");
    assert!(
        format.build(doc, &missing).is_err(),
        "SpecFormat: a layer that is not there was described"
    );
    let listed: BTreeSet<String> = spec
        .get("tokens")
        .and_then(Value::as_array)
        .expect("SpecFormat: the description has no tokens list")
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let mut used = BTreeSet::new();
    references(&spec, &mut used);
    assert!(
        used.is_subset(&listed),
        "SpecFormat: tokens used but not listed: {:?}",
        used.difference(&listed).collect::<Vec<_>>()
    );
}
