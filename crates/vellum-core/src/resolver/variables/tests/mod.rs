//! Resolver I: themes, variables, aliases, binding checks and the subtree cache. Everything here is `TEST DATA`.

mod bind;
mod cache;
mod theme;
mod values;

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};
use vellum_types::{Document, Node};

use super::Theme;

/// The two axes the tests use.
pub(super) fn axes() -> Json {
    json!({"Mode": ["Light", "Dark"], "Device": ["Phone", "Tablet"]})
}

/// A document with one canvas page holding `children`, and `extra` (`themes`, `variables`, ...) merged in.
pub(super) fn doc(extra: Json, children: Json) -> Document {
    let mut value = json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA resolver", "createdWith": "tests"},
        "pages": [{"id": "canvas0001", "name": "Canvas", "children": children}]
    });
    if let (Json::Object(map), Json::Object(extra)) = (&mut value, extra) {
        map.extend(extra);
    }
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{value}: {e}"))
}

/// A frame, 100 x 100 unless `extra` says otherwise.
pub(super) fn frame(id: &str, extra: Json, children: Json) -> Json {
    let mut value = json!({"type": "frame", "id": id, "name": format!("TEST DATA {id}"),
                           "width": 100, "height": 100, "children": children});
    if let (Json::Object(map), Json::Object(extra)) = (&mut value, extra) {
        map.extend(extra);
    }
    value
}

/// A rectangle, 10 x 10 unless `extra` says otherwise.
pub(super) fn rect(id: &str, extra: Json) -> Json {
    let mut value = json!({"type": "rect", "id": id, "name": format!("TEST DATA {id}"),
                           "width": 10, "height": 10});
    if let (Json::Object(map), Json::Object(extra)) = (&mut value, extra) {
        map.extend(extra);
    }
    value
}

pub(super) fn solid(variable: &str) -> Json {
    json!([{"type": "solid", "color": {"$var": variable}}])
}

/// The default theme of `doc`, with `choices` applied.
pub(super) fn theme(doc: &Document, choices: &[(&str, &str)]) -> Theme {
    let choices: BTreeMap<String, String> = choices
        .iter()
        .map(|(axis, value)| ((*axis).into(), (*value).into()))
        .collect();
    Theme::defaults(doc)
        .with(doc, &choices)
        .expect("axes and values the document has")
}

pub(super) fn node(value: Json) -> Node {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{value}: {e}"))
}
