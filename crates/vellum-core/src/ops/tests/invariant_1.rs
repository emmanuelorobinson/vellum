//! Invariant 1 as a property: arbitrary op batches either leave a valid document or are refused whole, and
//! a refused batch leaves no trace.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use serde_json::json;

use super::common::*;
use crate::ops::{Op, Place, apply, check_document};
use crate::ports::DocStore;

const IDS: &[&str] = &[
    "frame00001",
    "rect000001",
    "rect000002",
    "rect000003",
    "group00001",
    "ellipse001",
    "bool000001",
    "rect000004",
    "text000001",
    "slide00001",
    "croot00001",
    "ctext00001",
    "pnew000001",
    "pnew000002",
    "pnew000003",
    "missing001",
];
const PAGES: &[&str] = &[
    "canvas0001",
    "canvas0001",
    "canvas0001",
    "slides0001",
    "nopage0001",
];
const CONTAINERS: &[&str] = &[
    "frame00001",
    "group00001",
    "bool000001",
    "croot00001",
    "pnew000001",
];
const KINDS: &[&str] = &[
    "rect", "frame", "group", "boolean", "text", "note", "ellipse",
];
const VARS: &[&str] = &["color00001", "number0001", "color00002"];

fn pick(list: &'static [&'static str]) -> impl Strategy<Value = &'static str> {
    prop::sample::select(list)
}

fn place() -> impl Strategy<Value = Place> {
    prop_oneof![
        (pick(PAGES), 0..7usize).prop_map(|(p, i)| on_page(p, i)),
        (pick(CONTAINERS), 0..3usize).prop_map(|(n, i)| in_node(n, i)),
        (pick(IDS), 0..3usize).prop_map(|(n, i)| in_node(n, i)),
    ]
}

fn new_node() -> impl Strategy<Value = vellum_types::Node> {
    (pick(KINDS), pick(IDS), prop::option::of(pick(IDS)), any::<bool>()).prop_map(
        |(kind, root, child, uses_var)| {
            let mut value = json!({"type": kind, "id": root, "name": "TEST DATA", "width": 10, "height": 10});
            if kind == "boolean" {
                value["op"] = json!("union");
            }
            if kind == "text" || kind == "note" {
                value["content"] = if kind == "text" { json!([{"text": "x", "font": "Inter", "size": 14}]) } else { json!("x") };
            }
            if uses_var {
                value["opacity"] = json!({"$var": "number0001"});
            }
            if let Some(child) = child
                && matches!(kind, "frame" | "group" | "boolean")
            {
                value["children"] =
                    json!([{"type": "rect", "id": child, "name": "TEST DATA", "width": 1, "height": 1}]);
            }
            node(value)
        },
    )
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (place(), new_node()).prop_map(|(to, n)| Op::Insert { to, node: Box::new(n) }),
        (pick(IDS), prop_oneof![
            Just(json!({"name": "Renamed"})),
            Just(json!({"opacity": "bad"})),
            Just(json!({"rotation": 30})),
            Just(json!({"opacity": {"$var": "color00002"}})),
            Just(json!({"position": {"type": "absolute", "x": 1, "y": 2}})),
        ])
            .prop_map(|(n, p)| Op::Update { id: id(n), patch: patch(p) }),
        pick(IDS).prop_map(|n| Op::Delete { id: id(n) }),
        (pick(IDS), place()).prop_map(|(n, to)| Op::Move { id: id(n), to }),
        (pick(IDS), place()).prop_map(|(n, to)| Op::Copy { id: id(n), to }),
        (pick(IDS), new_node()).prop_map(|(n, node)| Op::Replace { id: id(n), node: Box::new(node) }),
        (pick(IDS), 0..6usize).prop_map(|(n, index)| Op::Reorder { id: id(n), index }),
        (prop::collection::vec(pick(IDS), 0..3), pick(IDS), any::<bool>()).prop_map(|(ids, w, frame)| {
            let ids = ids.into_iter().map(id).collect();
            if frame {
                Op::WrapInFrame { ids, frame: Box::new(shape("frame", w, at(4.0, 4.0))) }
            } else {
                Op::Group { ids, group: Box::new(shape("group", w, json!({}))) }
            }
        }),
        pick(IDS).prop_map(|n| Op::Ungroup { id: id(n) }),
        (prop::option::of(pick(VARS)), prop::option::of(pick(VARS)), pick(VARS)).prop_map(
            |(set, remove, alias)| Op::SetVariables {
                set: set
                    .map(|v| {
                        let body = json!({"type": "color", "name": "TEST DATA", "values": [{"$alias": alias}]});
                        BTreeMap::from([(var_id(v), variable(body))])
                    })
                    .unwrap_or_default(),
                remove: remove.map(|v| BTreeSet::from([var_id(v)])).unwrap_or_default(),
            }
        ),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1024, ..ProptestConfig::default() })]

    #[test]
    fn every_accepted_batch_leaves_a_valid_document_and_refused_ones_leave_no_trace(
        batches in prop::collection::vec(prop::collection::vec(op(), 1..3), 1..10)
    ) {
        let mut store = Store::new(document());
        let mut ids = Ids::default();
        let mut accepted = 0;
        for batch in batches {
            let before = store.snapshot();
            match apply(&mut store, &mut ids, tx(batch)) {
                Ok(_) => {
                    accepted += 1;
                    let problems = check_document(&store.doc);
                    prop_assert!(problems.is_empty(), "an accepted batch left {problems:?}");
                }
                Err(error) => {
                    prop_assert_eq!(&store.doc, &before, "refused ({}) but changed", error);
                }
            }
            prop_assert_eq!(store.applied, accepted);
        }
        while store.undo(&person(ALICE)).is_some() {}
        prop_assert_eq!(store.doc, document(), "undoing everything restores the start");
    }
}

#[test]
fn the_sample_document_is_valid_and_the_checker_finds_problems() {
    assert_eq!(check_document(&document()), Vec::<String>::new());
    let mut doc = document();
    let rect = doc.pages[0].children[1].clone();
    doc.pages[1].children.push(rect);
    let problems = check_document(&doc);
    assert!(
        problems.iter().any(|p| p.contains("rect000003")),
        "{problems:?}"
    );
}
