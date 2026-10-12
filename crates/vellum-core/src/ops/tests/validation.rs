//! Each invalid case: the error names the op, the path inside it and what is wrong, and nothing is applied.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use vellum_types::{NodeKind, PageKind};

use super::common::*;
use crate::ops::{Container, Op, OpErrorKind, Transaction, apply};

fn insert(to: crate::ops::Place, node: vellum_types::Node) -> Op {
    Op::Insert {
        to,
        node: Box::new(node),
    }
}

fn rect(node_id: &str) -> vellum_types::Node {
    shape("rect", node_id, json!({}))
}

#[track_caller]
fn expect(ops: Vec<Op>, op_index: usize, path: &str, kind: OpErrorKind) {
    let error = refused(ops);
    assert_eq!(
        (error.op_index, error.path.as_str(), &error.kind),
        (op_index, path, &kind),
        "{error}"
    );
}

#[test]
fn an_empty_transaction_is_refused() {
    expect(vec![], 0, "", OpErrorKind::Empty);
}

#[test]
fn unknown_targets() {
    expect(
        vec![insert(on_page("nopage0001", 0), rect("rnew000001"))],
        0,
        "to.parent",
        OpErrorKind::UnknownPage(page_id("nopage0001")),
    );
    expect(
        vec![Op::Delete {
            id: id("missing001"),
        }],
        0,
        "id",
        OpErrorKind::UnknownNode(id("missing001")),
    );
    expect(
        vec![Op::Move {
            id: id("rect000003"),
            to: in_node("missing001", 0),
        }],
        0,
        "to.parent",
        OpErrorKind::UnknownNode(id("missing001")),
    );
}

#[test]
fn indexes_past_the_end() {
    expect(
        vec![insert(in_node("frame00001", 3), rect("rnew000001"))],
        0,
        "to.index",
        OpErrorKind::IndexOutOfRange { index: 3, len: 2 },
    );
    expect(
        vec![Op::Reorder {
            id: id("rect000001"),
            index: 2,
        }],
        0,
        "index",
        OpErrorKind::IndexOutOfRange { index: 2, len: 1 },
    );
}

#[test]
fn ids_stay_unique_across_pages_and_components() {
    expect(
        vec![insert(on_page("canvas0001", 0), rect("rect000001"))],
        0,
        "node",
        OpErrorKind::DuplicateId(id("rect000001")),
    );
    let group = node(
        json!({"type": "group", "id": "gnew000001", "name": "G", "width": 1, "height": 1,
        "children": [{"type": "rect", "id": "rnew000001", "name": "a", "width": 1, "height": 1},
                     {"type": "rect", "id": "ctext00001", "name": "b", "width": 1, "height": 1}]}),
    );
    expect(
        vec![insert(on_page("canvas0001", 0), group)],
        0,
        "node.children[1]",
        OpErrorKind::DuplicateId(id("ctext00001")),
    );
}

#[test]
fn parent_kinds() {
    expect(
        vec![insert(in_node("rect000003", 0), rect("rnew000001"))],
        0,
        "to.parent",
        OpErrorKind::NotAContainer(id("rect000003")),
    );
    expect(
        vec![insert(
            in_node("bool000001", 0),
            shape("frame", "fnew000001", json!({})),
        )],
        0,
        "node",
        OpErrorKind::NotAllowedIn {
            id: id("fnew000001"),
            kind: NodeKind::Frame,
            parent: Container::Node(NodeKind::Boolean),
        },
    );
    expect(
        vec![insert(on_page("slides0001", 0), rect("rnew000001"))],
        0,
        "node",
        OpErrorKind::NotAllowedIn {
            id: id("rnew000001"),
            kind: NodeKind::Rect,
            parent: Container::Page(PageKind::Slides),
        },
    );
    expect(
        vec![Op::Move {
            id: id("frame00001"),
            to: in_node("bool000001", 0),
        }],
        0,
        "id",
        OpErrorKind::NotAllowedIn {
            id: id("frame00001"),
            kind: NodeKind::Frame,
            parent: Container::Node(NodeKind::Boolean),
        },
    );
}

#[test]
fn cycles() {
    expect(
        vec![Op::Move {
            id: id("frame00001"),
            to: in_node("frame00001", 0),
        }],
        0,
        "to.parent",
        OpErrorKind::Cycle(id("frame00001")),
    );
    let ops = vec![
        insert(
            in_node("frame00001", 0),
            shape("frame", "fnew000001", json!({})),
        ),
        Op::Move {
            id: id("frame00001"),
            to: in_node("fnew000001", 0),
        },
    ];
    expect(ops, 1, "to.parent", OpErrorKind::Cycle(id("frame00001")));
}

#[test]
fn page_and_component_boundaries() {
    expect(
        vec![Op::Move {
            id: id("rect000003"),
            to: in_node("croot00001", 0),
        }],
        0,
        "to.parent",
        OpErrorKind::CrossesBoundary(id("rect000003")),
    );
    expect(
        vec![Op::Move {
            id: id("ctext00001"),
            to: on_page("canvas0001", 0),
        }],
        0,
        "to.parent",
        OpErrorKind::CrossesBoundary(id("ctext00001")),
    );
    expect(
        vec![Op::Delete {
            id: id("croot00001"),
        }],
        0,
        "id",
        OpErrorKind::ComponentRoot(id("croot00001")),
    );
    expect(
        vec![Op::Group {
            ids: vec![id("rect000003"), id("slide00001")],
            group: Box::new(shape("group", "gnew000001", json!({}))),
        }],
        0,
        "ids[1]",
        OpErrorKind::NotSiblings(id("slide00001")),
    );
}

#[test]
fn wrappers() {
    let wrap = |ids: Vec<&str>, wrapper| Op::WrapInFrame {
        ids: ids.into_iter().map(id).collect(),
        frame: Box::new(wrapper),
    };
    expect(
        vec![wrap(vec![], shape("frame", "fnew000001", json!({})))],
        0,
        "ids",
        OpErrorKind::NoTargets,
    );
    expect(
        vec![wrap(
            vec!["rect000001", "rect000001"],
            shape("frame", "fnew000001", json!({})),
        )],
        0,
        "ids[1]",
        OpErrorKind::Repeated(id("rect000001")),
    );
    expect(
        vec![wrap(
            vec!["rect000001", "rect000003"],
            shape("frame", "fnew000001", json!({})),
        )],
        0,
        "ids[1]",
        OpErrorKind::NotSiblings(id("rect000003")),
    );
    expect(
        vec![wrap(vec!["rect000001"], rect("rnew000001"))],
        0,
        "frame",
        OpErrorKind::WrongKind {
            expected: NodeKind::Frame,
            found: NodeKind::Rect,
        },
    );
    let full = node(
        json!({"type": "frame", "id": "fnew000001", "name": "F", "width": 1, "height": 1,
        "children": [{"type": "rect", "id": "rnew000001", "name": "a", "width": 1, "height": 1}]}),
    );
    expect(
        vec![wrap(vec!["rect000001"], full)],
        0,
        "frame",
        OpErrorKind::WrapperNotEmpty,
    );
    expect(
        vec![wrap(
            vec!["rect000001"],
            shape("frame", "fnew000001", json!({"rotation": 45})),
        )],
        0,
        "frame",
        OpErrorKind::Transformed(id("fnew000001")),
    );
}

#[test]
fn ungroup_needs_a_plain_group() {
    expect(
        vec![Op::Ungroup {
            id: id("frame00001"),
        }],
        0,
        "id",
        OpErrorKind::WrongKind {
            expected: NodeKind::Group,
            found: NodeKind::Frame,
        },
    );
    expect(
        vec![
            Op::Update {
                id: id("group00001"),
                patch: patch(json!({"flipX": true})),
            },
            Op::Ungroup {
                id: id("group00001"),
            },
        ],
        1,
        "id",
        OpErrorKind::Transformed(id("group00001")),
    );
}

#[test]
fn a_bad_patch_names_its_property() {
    let error = refused(vec![Op::Update {
        id: id("rect000001"),
        patch: patch(json!({"name": "ok", "opacity": "half"})),
    }]);
    assert_eq!((error.op_index, error.path.as_str()), (0, "patch.opacity"));
    assert!(matches!(error.kind, OpErrorKind::InvalidPatch { .. }));
    assert_eq!(error.location(), "ops[0].patch.opacity");
}

#[test]
fn variable_references_must_resolve() {
    expect(
        vec![insert(
            on_page("canvas0001", 0),
            shape(
                "rect",
                "rnew000001",
                json!({"opacity": {"$var": "nothere01"}}),
            ),
        )],
        0,
        "node",
        OpErrorKind::UnknownVariable(var_id("nothere01")),
    );
    expect(
        vec![Op::Update {
            id: id("rect000001"),
            patch: patch(json!({"opacity": {"$var": "nothere01"}})),
        }],
        0,
        "patch",
        OpErrorKind::UnknownVariable(var_id("nothere01")),
    );
    expect(
        vec![Op::SetVariables {
            set: BTreeMap::new(),
            remove: BTreeSet::from([var_id("color00001")]),
        }],
        0,
        "remove.color00001",
        OpErrorKind::VariableInUse(var_id("color00001")),
    );
    expect(
        vec![Op::SetVariables {
            set: BTreeMap::new(),
            remove: BTreeSet::from([var_id("nothere01")]),
        }],
        0,
        "remove.nothere01",
        OpErrorKind::UnknownVariable(var_id("nothere01")),
    );
}

#[test]
fn aliases_must_resolve_to_the_same_type_without_cycles() {
    let set = |entries: Vec<(&str, serde_json::Value)>| Op::SetVariables {
        set: entries
            .into_iter()
            .map(|(k, v)| (var_id(k), variable(v)))
            .collect(),
        remove: BTreeSet::new(),
    };
    expect(
        vec![set(vec![(
            "color00002",
            json!({"type": "color", "name": "a", "values": [{"$alias": "nothere01"}]}),
        )])],
        0,
        "set.color00002",
        OpErrorKind::UnknownVariable(var_id("nothere01")),
    );
    expect(
        vec![set(vec![(
            "color00002",
            json!({"type": "color", "name": "a", "values": [{"$alias": "number0001"}]}),
        )])],
        0,
        "set.color00002",
        OpErrorKind::AliasType {
            alias: var_id("number0001"),
            expected: vellum_types::VariableType::Color,
            found: vellum_types::VariableType::Number,
        },
    );
    expect(
        vec![set(vec![
            (
                "color00001",
                json!({"type": "color", "name": "a", "values": [{"$alias": "color00002"}]}),
            ),
            (
                "color00002",
                json!({"type": "color", "name": "b", "values": [{"$alias": "color00001"}]}),
            ),
        ])],
        0,
        "set.color00001",
        OpErrorKind::AliasCycle(var_id("color00001")),
    );
}

#[test]
fn a_refused_batch_leaves_no_trace() {
    let mut store = Store::new(document());
    let ops = vec![
        Op::Delete {
            id: id("rect000003"),
        },
        Op::Update {
            id: id("frame00001"),
            patch: patch(json!({"name": "Changed"})),
        },
        Op::Delete {
            id: id("rect000003"),
        },
    ];
    let error = apply(
        &mut store,
        &mut Ids::default(),
        Transaction::new(alice(), ops),
    )
    .expect_err("the third op deletes a node that is gone");
    assert_eq!(error.op_index, 2);
    assert_eq!(error.kind, OpErrorKind::UnknownNode(id("rect000003")));
    assert_eq!(store.applied, 0, "the store never saw the batch");
    assert_eq!(store.doc, document());
}

#[test]
fn errors_read_as_plain_language() {
    let error = refused(vec![Op::Delete {
        id: id("missing001"),
    }]);
    assert_eq!(
        error.to_string(),
        "Change 1 was refused: there is no layer missing001."
    );
    assert_eq!(refused(vec![]).to_string(), "The change has nothing in it.");
}
