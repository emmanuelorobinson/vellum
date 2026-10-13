//! Three-way merge by node id and property. The properties over random documents (merge(b, x, b) == x,
//! merge(b, b, y) == y, commutativity) are in `tests/diff_merge.rs`; these are the cases by name.

use serde_json::json;
use vellum_types::{ComponentId, Document, Node, NodeKind, PageKind};

use super::{Conflict, Side, merge};
use crate::ops::tests::common::*;
use crate::ops::{Op, check_document, find};

fn edited(base: &Document, ops: Vec<Op>) -> Document {
    planned(base, ops)
        .unwrap_or_else(|e| panic!("refused: {e} at {}", e.location()))
        .document
}

fn merged(base: &Document, ours: &Document, theirs: &Document) -> (Document, Vec<Conflict>) {
    let m = merge(base, ours, theirs).unwrap_or_else(|e| panic!("merge failed: {e:?}"));
    assert!(
        check_document(&m.document).is_empty(),
        "the merge is not a valid document: {:?}",
        check_document(&m.document)
    );
    (m.document, m.conflicts)
}

fn rename(node: &str, name: &str) -> Op {
    Op::Update {
        id: id(node),
        patch: patch(json!({ "name": name })),
    }
}

fn opacity(node: &str, value: f64) -> Op {
    Op::Update {
        id: id(node),
        patch: patch(json!({ "opacity": value })),
    }
}

fn name_of(doc: &Document, node: &str) -> String {
    find(doc, &id(node)).expect("the node").name().to_string()
}

fn conflict_at<'a>(doc: &'a Document, node: &str) -> &'a vellum_types::ConflictNode {
    match find(doc, &id(node)) {
        Some(Node::Conflict(c)) => c,
        other => panic!("{node} is not a conflict node: {other:?}"),
    }
}

#[test]
fn unchanged_sides_give_the_base() {
    let base = document();
    let (doc, conflicts) = merged(&base, &base, &base);
    assert_eq!(doc, base);
    assert!(conflicts.is_empty());
}

#[test]
fn one_side_changed_gives_that_side() {
    let base = document();
    let x = edited(
        &base,
        vec![
            rename("rect000001", "TEST DATA x"),
            Op::Delete {
                id: id("group00001"),
            },
            Op::Move {
                id: id("rect000003"),
                to: in_node("frame00001", 0),
            },
        ],
    );
    assert_eq!(merged(&base, &x, &base), (x.clone(), vec![]));
    assert_eq!(merged(&base, &base, &x), (x, vec![]));
}

#[test]
fn edits_to_different_nodes_and_properties_merge_cleanly() {
    let base = document();
    let ours = edited(
        &base,
        vec![
            rename("rect000001", "TEST DATA ours"),
            opacity("rect000002", 0.5),
        ],
    );
    let theirs = edited(
        &base,
        vec![
            opacity("rect000001", 0.25),
            Op::Insert {
                to: in_node("frame00001", 2),
                node: Box::new(shape("rect", "theirs0001", json!({}))),
            },
        ],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    let a = find(&doc, &id("rect000001")).expect("a");
    assert_eq!(a.name(), "TEST DATA ours");
    assert_eq!(a.opacity(), &vellum_types::Value::Literal(0.25));
    assert_eq!(name_of(&doc, "rect000002"), "B");
    assert_eq!(
        child_ids(&doc, "frame00001"),
        ["rect000001", "rect000002", "theirs0001"]
    );
    // And the other way round gives the same document.
    assert_eq!(merged(&base, &theirs, &ours), (doc, vec![]));
}

#[test]
fn the_same_property_changed_on_both_sides_leaves_a_conflict_node_with_both_versions() {
    let base = document();
    let ours = edited(
        &base,
        vec![
            rename("rect000001", "TEST DATA ours"),
            opacity("rect000001", 0.5),
        ],
    );
    let theirs = edited(&base, vec![rename("rect000001", "TEST DATA theirs")]);
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(
        conflicts,
        [Conflict::Properties {
            id: id("rect000001"),
            properties: vec!["name".into()],
        }]
    );
    let c = conflict_at(&doc, "rect000001");
    assert_eq!(c.properties, ["name"]);
    let (o, t) = (
        c.ours.as_deref().expect("ours"),
        c.theirs.as_deref().expect("theirs"),
    );
    assert_eq!((o.kind(), t.kind()), (NodeKind::Rect, NodeKind::Rect));
    assert_eq!((o.name(), t.name()), ("TEST DATA ours", "TEST DATA theirs"));
    // What did not conflict is in both versions.
    assert_eq!(o.opacity(), &vellum_types::Value::Literal(0.5));
    assert_eq!(t.opacity(), &vellum_types::Value::Literal(0.5));
    // The conflict takes the node's place.
    assert_eq!(child_ids(&doc, "frame00001"), ["rect000001", "rect000002"]);
}

#[test]
fn a_conflicted_container_keeps_its_merged_children() {
    let base = document();
    let ours = edited(
        &base,
        vec![
            rename("frame00001", "TEST DATA ours"),
            rename("rect000001", "TEST DATA kid"),
        ],
    );
    let theirs = edited(
        &base,
        vec![
            rename("frame00001", "TEST DATA theirs"),
            Op::Delete {
                id: id("rect000002"),
            },
        ],
    );
    let (doc, _) = merged(&base, &ours, &theirs);
    let c = conflict_at(&doc, "frame00001");
    assert_eq!(c.children.len(), 1);
    assert_eq!(c.children[0].name(), "TEST DATA kid");
    assert!(
        c.ours.as_ref().expect("ours").children().is_empty(),
        "versions hold no children"
    );
}

#[test]
fn removed_on_one_side_and_changed_on_the_other_keeps_the_change_in_a_conflict_node() {
    let base = document();
    let ours = edited(
        &base,
        vec![Op::Delete {
            id: id("frame00001"),
        }],
    );
    let theirs = edited(
        &base,
        vec![rename("rect000002", "TEST DATA changed inside")],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(
        conflicts,
        [Conflict::Removed {
            id: id("frame00001"),
            by: Side::Ours,
        }]
    );
    let c = conflict_at(&doc, "frame00001");
    assert!(c.ours.is_none());
    assert_eq!(c.theirs.as_ref().expect("theirs").kind(), NodeKind::Frame);
    assert_eq!(c.children.len(), 2, "their whole version lives on");
    assert_eq!(name_of(&doc, "rect000002"), "TEST DATA changed inside");

    // Removed on one side and untouched on the other: removed.
    let theirs = edited(&base, vec![rename("rect000003", "TEST DATA elsewhere")]);
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts.is_empty());
    assert!(find(&doc, &id("frame00001")).is_none());
}

#[test]
fn moves_merge_by_parent_and_order() {
    let base = document();
    let ours = edited(
        &base,
        vec![Op::Move {
            id: id("rect000003"),
            to: in_node("group00001", 0),
        }],
    );
    let theirs = edited(
        &base,
        vec![Op::Reorder {
            id: id("rect000002"),
            index: 0,
        }],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts.is_empty());
    assert_eq!(child_ids(&doc, "group00001"), ["rect000003", "ellipse001"]);
    assert_eq!(child_ids(&doc, "frame00001"), ["rect000002", "rect000001"]);

    // Both moved one node to different places: ours wins, and it is reported.
    let theirs = edited(
        &base,
        vec![Op::Move {
            id: id("rect000003"),
            to: in_node("frame00001", 0),
        }],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(
        conflicts,
        [Conflict::Moved {
            id: id("rect000003")
        }]
    );
    assert_eq!(child_ids(&doc, "group00001"), ["rect000003", "ellipse001"]);
}

#[test]
fn moves_that_would_make_a_cycle_keep_our_tree() {
    let base = document();
    let ours = edited(
        &base,
        vec![Op::Move {
            id: id("group00001"),
            to: in_node("frame00001", 0),
        }],
    );
    let theirs = edited(
        &base,
        vec![Op::Move {
            id: id("frame00001"),
            to: in_node("group00001", 0),
        }],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(!conflicts.is_empty());
    assert_eq!(child_ids(&doc, "frame00001")[0], "group00001");
}

#[test]
fn inserts_at_the_same_place_land_in_the_same_order_whichever_side_is_ours() {
    let base = document();
    let insert = |node_id: &str| {
        edited(
            &base,
            vec![Op::Insert {
                to: in_node("frame00001", 1),
                node: Box::new(shape("rect", node_id, json!({}))),
            }],
        )
    };
    let (x, y) = (insert("xnew000001"), insert("ynew000001"));
    let (one, _) = merged(&base, &x, &y);
    let (two, _) = merged(&base, &y, &x);
    assert_eq!(one, two);
    assert_eq!(
        child_ids(&one, "frame00001"),
        ["rect000001", "xnew000001", "ynew000001", "rect000002"]
    );
}

#[test]
fn variables_shell_and_pages_merge_by_entry() {
    let base = document();
    let mut ours = edited(
        &base,
        vec![Op::UpdatePage {
            id: page_id("slides0001"),
            name: "TEST DATA ours".into(),
            kind: PageKind::Slides,
        }],
    );
    ours.themes.insert("mode".into(), vec!["light".into()]);
    let mut theirs = edited(
        &base,
        vec![Op::MovePage {
            id: page_id("slides0001"),
            index: 0,
        }],
    );
    let renamed =
        variable(json!({"type": "number", "name": "TEST DATA theirs", "values": [{"value": 16}]}));
    theirs
        .variables
        .insert(var_id("number0001"), renamed.clone());
    theirs.meta.name = "TEST DATA theirs".into();
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!(doc.pages[0].name, "TEST DATA ours");
    assert!(doc.themes.contains_key("mode"));
    assert_eq!(doc.variables[&var_id("number0001")], renamed);
    assert_eq!(doc.meta.name, "TEST DATA theirs");

    // The same entry changed both ways: ours is kept and the conflict is reported.
    let mut ours = base.clone();
    ours.meta.name = "TEST DATA ours".into();
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(doc.meta.name, "TEST DATA ours");
    assert_eq!(
        conflicts,
        [Conflict::Entry {
            path: "meta.name".into()
        }]
    );
}

#[test]
fn a_variable_removed_on_one_side_and_used_on_the_other_stays() {
    let base = document();
    let ours = edited(
        &base,
        vec![
            Op::Update {
                id: id("text000001"),
                patch: patch(json!({"fills": null})),
            },
            Op::SetVariables {
                set: Default::default(),
                remove: [var_id("color00001")].into(),
            },
        ],
    );
    let theirs = edited(
        &base,
        vec![Op::Update {
            id: id("rect000001"),
            patch: patch(json!({"fills": [{"type": "solid", "color": {"$var": "color00001"}}]})),
        }],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(doc.variables.contains_key(&var_id("color00001")));
    assert_eq!(
        conflicts,
        [Conflict::Entry {
            path: "variables.color00001".into()
        }]
    );
}

#[test]
fn a_page_removed_on_one_side_and_drawn_on_on_the_other_stays() {
    let base = document();
    let ours = edited(
        &base,
        vec![Op::DeletePage {
            id: page_id("slides0001"),
        }],
    );
    let theirs = edited(
        &base,
        vec![Op::Insert {
            to: on_page("slides0001", 1),
            node: Box::new(shape("frame", "slide00002", json!({}))),
        }],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(doc.pages.iter().any(|p| p.id == page_id("slides0001")));
    assert!(find(&doc, &id("slide00002")).is_some());
    assert!(!conflicts.is_empty());
}

#[test]
fn component_trees_merge_by_node_and_definitions_by_field() {
    let base = document();
    let comp = ComponentId::new("comp000001").expect("id");
    let mut ours = edited(&base, vec![rename("ctext00001", "TEST DATA ours")]);
    ours.components.get_mut(&comp).expect("c").name = "TEST DATA Ours button".into();
    let theirs = edited(
        &base,
        vec![Op::Insert {
            to: in_node("croot00001", 1),
            node: Box::new(shape("rect", "cnew000001", json!({}))),
        }],
    );
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    let c = &doc.components[&comp];
    assert_eq!(c.name, "TEST DATA Ours button");
    assert_eq!(
        c.root
            .children()
            .iter()
            .map(|n| n.id().to_string())
            .collect::<Vec<_>>(),
        ["ctext00001", "cnew000001"]
    );
    assert_eq!(name_of(&doc, "ctext00001"), "TEST DATA ours");
}

#[test]
fn a_kind_change_on_one_side_against_a_property_change_on_the_other_conflicts() {
    let base = document();
    let ours = edited(
        &base,
        vec![Op::Replace {
            id: id("rect000003"),
            node: Box::new(shape("ellipse", "rect000003", json!({}))),
        }],
    );
    let theirs = edited(&base, vec![opacity("rect000003", 0.5)]);
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(
        matches!(&conflicts[..], [Conflict::Properties { properties, .. }] if properties.contains(&"type".to_string())),
        "{conflicts:?}"
    );
    let c = conflict_at(&doc, "rect000003");
    assert_eq!(c.ours.as_ref().expect("ours").kind(), NodeKind::Ellipse);
    assert_eq!(c.theirs.as_ref().expect("theirs").kind(), NodeKind::Rect);

    // Alone, a kind change merges like any change.
    let theirs = edited(&base, vec![opacity("rect000001", 0.5)]);
    let (doc, conflicts) = merged(&base, &ours, &theirs);
    assert!(conflicts.is_empty());
    assert_eq!(
        find(&doc, &id("rect000003")).expect("n").kind(),
        NodeKind::Ellipse
    );
}

#[test]
fn a_document_with_conflicts_merges_again() {
    // A file with conflict nodes in it is an ordinary document: it can be merged and diffed again.
    let base = document();
    let ours = edited(&base, vec![rename("rect000001", "TEST DATA ours")]);
    let theirs = edited(&base, vec![rename("rect000001", "TEST DATA theirs")]);
    let (conflicted, _) = merged(&base, &ours, &theirs);
    let later = edited(&conflicted, vec![rename("rect000002", "TEST DATA later")]);
    let (doc, conflicts) = merged(&conflicted, &later, &conflicted);
    assert_eq!(doc, later);
    assert!(conflicts.is_empty());
    // Keeping one version is a replace with the same id.
    let c = conflict_at(&conflicted, "rect000001").clone();
    let resolved = edited(
        &conflicted,
        vec![Op::Replace {
            id: id("rect000001"),
            node: c.theirs.expect("theirs"),
        }],
    );
    assert_eq!(name_of(&resolved, "rect000001"), "TEST DATA theirs");
}
