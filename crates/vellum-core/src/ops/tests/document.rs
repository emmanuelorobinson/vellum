//! Ops on the parts of a document that are not nodes or variables: pages, the shell (meta, imports, themes,
//! styles, fonts) and component definitions. The diff (Prompt 15) needs every part of a document to be
//! reachable through ops, since an external change is applied as ops (ARCHITECTURE §4.5).

use std::collections::BTreeMap;

use serde_json::json;
use vellum_types::{Component, ComponentId, Document, NodeKind, Page, PageKind};

use super::common::*;
use crate::ops::{Op, OpErrorKind, Shell, apply_edits};

fn page(value: serde_json::Value) -> Page {
    serde_json::from_value(value).expect("a valid page")
}

fn component(value: serde_json::Value) -> Component {
    serde_json::from_value(value).expect("a valid component")
}

fn comp_id(text: &str) -> ComponentId {
    ComponentId::new(text).expect("a valid id")
}

fn page_ids(doc: &Document) -> Vec<String> {
    doc.pages.iter().map(|p| p.id.to_string()).collect()
}

/// Plans `ops`, checks the edits reach the plan's document and the inverse restores the original.
fn round_trip(ops: Vec<Op>) -> Document {
    let original = document();
    let plan =
        planned(&original, ops).unwrap_or_else(|e| panic!("refused: {e} at {}", e.location()));
    let mut doc = original.clone();
    apply_edits(&mut doc, &plan.edits, alice()).expect("the edits apply");
    assert_eq!(doc, plan.document, "the edits are not the plan");
    apply_edits(&mut doc, &plan.inverse, alice()).expect("the inverse applies");
    assert_eq!(doc, original, "the inverse did not restore the document");
    plan.document
}

#[test]
fn insert_page_adds_a_page_with_its_nodes() {
    let doc = round_trip(vec![Op::InsertPage {
        index: 1,
        page: Box::new(page(
            json!({"id": "canvas0002", "name": "TEST DATA More", "children": [
                {"type": "rect", "id": "rnew000001", "name": "R", "width": 1, "height": 1}
            ]}),
        )),
    }]);
    assert_eq!(page_ids(&doc), ["canvas0001", "canvas0002", "slides0001"]);
    assert!(find(&doc, "rnew000001").is_some());
}

#[test]
fn insert_page_refuses_a_taken_page_id_or_node_id() {
    let error = refused(vec![Op::InsertPage {
        index: 0,
        page: Box::new(page(json!({"id": "canvas0001", "name": "Again"}))),
    }]);
    assert_eq!(
        error.kind,
        OpErrorKind::DuplicatePage(page_id("canvas0001"))
    );
    let error = refused(vec![Op::InsertPage {
        index: 0,
        page: Box::new(page(json!({"id": "canvas0002", "name": "P", "children": [
            {"type": "rect", "id": "rect000001", "name": "R", "width": 1, "height": 1}
        ]}))),
    }]);
    assert_eq!(error.kind, OpErrorKind::DuplicateId(id("rect000001")));
    let error = refused(vec![Op::InsertPage {
        index: 3,
        page: Box::new(page(json!({"id": "canvas0002", "name": "P"}))),
    }]);
    assert_eq!(
        error.kind,
        OpErrorKind::IndexOutOfRange { index: 3, len: 2 }
    );
}

#[test]
fn delete_page_removes_it_and_everything_on_it() {
    let doc = round_trip(vec![Op::DeletePage {
        id: page_id("slides0001"),
    }]);
    assert_eq!(page_ids(&doc), ["canvas0001"]);
    assert!(find(&doc, "slide00001").is_none());
    let error = refused(vec![Op::DeletePage {
        id: page_id("nopage0001"),
    }]);
    assert_eq!(error.kind, OpErrorKind::UnknownPage(page_id("nopage0001")));
}

#[test]
fn update_page_renames_and_changes_kind_when_its_nodes_allow() {
    let doc = round_trip(vec![Op::UpdatePage {
        id: page_id("slides0001"),
        name: "TEST DATA Deck".into(),
        kind: PageKind::Canvas,
    }]);
    assert_eq!(doc.pages[1].name, "TEST DATA Deck");
    assert_eq!(doc.pages[1].kind, PageKind::Canvas);
    assert_eq!(doc.pages[1].children.len(), 1, "a page keeps its nodes");

    // A slides page holds frames and annotations only.
    let error = refused(vec![Op::UpdatePage {
        id: page_id("canvas0001"),
        name: "Canvas".into(),
        kind: PageKind::Slides,
    }]);
    assert!(
        matches!(
            error.kind,
            OpErrorKind::NotAllowedIn {
                kind: NodeKind::Rect,
                ..
            }
        ),
        "{:?}",
        error.kind
    );
}

#[test]
fn move_page_reorders_pages() {
    let doc = round_trip(vec![Op::MovePage {
        id: page_id("slides0001"),
        index: 0,
    }]);
    assert_eq!(page_ids(&doc), ["slides0001", "canvas0001"]);
    let error = refused(vec![Op::MovePage {
        id: page_id("slides0001"),
        index: 2,
    }]);
    assert_eq!(
        error.kind,
        OpErrorKind::IndexOutOfRange { index: 2, len: 1 }
    );
}

#[test]
fn set_shell_replaces_meta_imports_themes_styles_and_fonts() {
    let mut shell = Shell::of(&document());
    shell.meta.name = "TEST DATA renamed".into();
    shell.themes = BTreeMap::from([("mode".into(), vec!["light".into(), "dark".into()])]);
    shell.imports = BTreeMap::from([("ui".into(), "ui.lib.vellum".into())]);
    let doc = round_trip(vec![Op::SetShell {
        shell: Box::new(shell.clone()),
    }]);
    assert_eq!(Shell::of(&doc), shell);
    assert_eq!(
        doc.pages,
        document().pages,
        "the shell leaves the pages alone"
    );
}

#[test]
fn set_component_adds_replaces_and_removes_a_definition() {
    let button = component(json!({"name": "TEST DATA Chip", "root": {
        "type": "frame", "id": "chiproot01", "name": "Chip", "width": 10, "height": 10
    }}));
    let doc = round_trip(vec![Op::SetComponent {
        id: comp_id("comp000002"),
        component: Some(Box::new(button.clone())),
    }]);
    assert_eq!(doc.components[&comp_id("comp000002")], button);

    let doc = round_trip(vec![Op::SetComponent {
        id: comp_id("comp000001"),
        component: Some(Box::new(button.clone())),
    }]);
    assert_eq!(doc.components[&comp_id("comp000001")], button);
    assert!(find(&doc, "ctext00001").is_none(), "the old tree is gone");

    let doc = round_trip(vec![Op::SetComponent {
        id: comp_id("comp000001"),
        component: None,
    }]);
    assert!(doc.components.is_empty());
}

#[test]
fn set_component_refuses_a_root_that_is_not_a_frame_or_ids_in_use() {
    let error = refused(vec![Op::SetComponent {
        id: comp_id("comp000002"),
        component: Some(Box::new(component(json!({"name": "X", "root": {
            "type": "rect", "id": "xroot00001", "name": "X", "width": 1, "height": 1
        }})))),
    }]);
    assert!(
        matches!(error.kind, OpErrorKind::WrongKind { .. }),
        "{:?}",
        error.kind
    );
    let error = refused(vec![Op::SetComponent {
        id: comp_id("comp000002"),
        component: Some(Box::new(component(json!({"name": "X", "root": {
            "type": "frame", "id": "rect000001", "name": "X", "width": 1, "height": 1
        }})))),
    }]);
    assert_eq!(error.kind, OpErrorKind::DuplicateId(id("rect000001")));
}

#[test]
fn update_component_changes_the_definition_and_keeps_its_tree() {
    let doc = round_trip(vec![Op::UpdateComponent {
        id: comp_id("comp000001"),
        name: "TEST DATA Primary button".into(),
        props: BTreeMap::from([(
            "label".into(),
            serde_json::from_value(json!({"type": "text", "default": "OK"})).expect("a prop"),
        )]),
        variants: Vec::new(),
    }]);
    let c = &doc.components[&comp_id("comp000001")];
    assert_eq!(c.name, "TEST DATA Primary button");
    assert_eq!(c.props.len(), 1);
    assert_eq!(c.root, document().components[&comp_id("comp000001")].root);
}

fn instance(node_id: &str, component: &str) -> vellum_types::Node {
    node(
        json!({"type": "instance", "id": node_id, "name": "TEST DATA i", "width": "hug",
                "height": "hug", "componentId": component}),
    )
}

/// Component references are checked once, against the document the whole transaction leaves, so ops may
/// arrive in any order that ends valid. The diff relies on this.
#[test]
fn references_are_checked_once_per_transaction() {
    let doc = round_trip(vec![
        Op::Insert {
            to: on_page("canvas0001", 0),
            node: Box::new(instance("inst000001", "comp000002")),
        },
        Op::SetComponent {
            id: comp_id("comp000002"),
            component: Some(Box::new(component(
                json!({"name": "TEST DATA Later", "root": {
                    "type": "frame", "id": "lroot00001", "name": "L", "width": 1, "height": 1
                }}),
            ))),
        },
    ]);
    assert!(find(&doc, "inst000001").is_some());
}

/// A transaction that ends with a broken reference is refused, and the error names the op after which the
/// references were first broken, as before.
#[test]
fn a_transaction_that_ends_broken_names_the_first_op_that_broke_it() {
    let error = refused(vec![
        Op::Update {
            id: id("rect000001"),
            patch: patch(json!({"name": "TEST DATA fine"})),
        },
        Op::Insert {
            to: on_page("canvas0001", 0),
            node: Box::new(instance("inst000001", "comp000009")),
        },
        Op::Update {
            id: id("rect000002"),
            patch: patch(json!({"name": "TEST DATA fine too"})),
        },
    ]);
    assert_eq!(error.op_index, 1);
    assert_eq!(
        error.kind,
        OpErrorKind::UnknownComponent(comp_id("comp000009"))
    );

    // Removing a component still in use is refused the same way.
    let mut used = document();
    used.pages[0]
        .children
        .push(instance("inst000001", "comp000001"));
    let error = planned(
        &used,
        vec![Op::SetComponent {
            id: comp_id("comp000001"),
            component: None,
        }],
    )
    .expect_err("the component is in use");
    assert_eq!(
        error.kind,
        OpErrorKind::UnknownComponent(comp_id("comp000001"))
    );
}
