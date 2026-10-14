//! The subtree cache: a second resolve computes nothing, a change recomputes only what it reaches, and the
//! cached result always equals a fresh one.

use std::sync::Arc;

use serde_json::{Value as Json, json};
use vellum_types::Document;

use super::{axes, doc, frame, rect, solid};
use crate::ops::tests::common::{self as c};
use crate::ops::{Op, Shell};
use crate::resolver::variables::{ResolvedNode, Resolver};

fn var(id: &str) -> Json {
    json!({"$var": id})
}

/// `root000001` holds `left000001` (leaves A and B) and `right00001` (leaf C). Eight nodes with a `$var` or
/// a theme to resolve, six in all: root, left, right and three leaves.
fn sample() -> Document {
    doc(
        json!({"themes": axes(), "variables": {
            "surface001": {"type": "color", "name": "color/surface", "values": [
                {"value": "#FFFFFF"}, {"value": "#111111", "when": {"Mode": "Dark"}}
            ]},
            "gap0000001": {"type": "number", "name": "space/gap", "values": [{"value": 4}]},
            "gap0000002": {"type": "number", "name": "space/alias", "values": [{"$alias": "gap0000001"}]}
        }}),
        json!([frame(
            "root000001",
            json!({"fills": solid("surface001")}),
            json!([
                frame(
                    "left000001",
                    json!({"gap": var("gap0000002")}),
                    json!([
                        rect("leafA00001", json!({"fills": solid("surface001")})),
                        rect("leafB00001", json!({}))
                    ])
                ),
                frame(
                    "right00001",
                    json!({}),
                    json!([rect("leafC00001", json!({"opacity": var("gap0000001")}))])
                )
            ])
        )]),
    )
}

fn apply(doc: &Document, ops: Vec<Op>) -> (Document, crate::ports::ChangeSet) {
    let plan = crate::ops::plan(doc, &c::tx(ops), &mut c::Ids::default())
        .unwrap_or_else(|e| panic!("refused: {e} at {}", e.location()));
    (plan.document, plan.changes)
}

fn update(id: &str, patch: Json) -> Op {
    Op::Update {
        id: c::id(id),
        patch: c::patch(patch),
    }
}

fn fresh(doc: &Document) -> Vec<Arc<ResolvedNode>> {
    Resolver::new().resolve_page(doc, &doc.pages[0])
}

/// Resolves `after` through a cache primed with `before`, and checks it against a fresh resolution.
fn step(
    resolver: &mut Resolver,
    before: &Document,
    after: &Document,
    changes: &crate::ports::ChangeSet,
) -> (Vec<Arc<ResolvedNode>>, u64) {
    let _ = before;
    resolver.invalidate(after, changes);
    let computed = resolver.computed();
    let tree = resolver.resolve_page(after, &after.pages[0]);
    assert_eq!(tree, fresh(after), "the cached tree equals a fresh one");
    (tree, resolver.computed() - computed)
}

#[test]
fn a_second_resolve_computes_nothing() {
    let d = sample();
    let mut resolver = Resolver::new();
    let first = resolver.resolve_page(&d, &d.pages[0]);
    assert_eq!(resolver.computed(), 6);
    let second = resolver.resolve_page(&d, &d.pages[0]);
    assert_eq!(resolver.computed(), 6, "every subtree came from the cache");
    assert!(Arc::ptr_eq(&first[0], &second[0]));
    assert_eq!(resolver.cached(), 6);
}

#[test]
fn an_update_recomputes_only_the_node_it_changed() {
    let d = sample();
    let mut resolver = Resolver::new();
    let first = resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(
        &d,
        vec![update("leafB00001", json!({"name": "TEST DATA renamed"}))],
    );
    let (second, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 1);
    let (old_left, new_left) = (&first[0].children[0], &second[0].children[0]);
    assert!(
        !Arc::ptr_eq(old_left, new_left),
        "the path to the change is rebuilt"
    );
    assert!(
        Arc::ptr_eq(&old_left.children[0], &new_left.children[0]),
        "its sibling is reused"
    );
    assert!(
        Arc::ptr_eq(&first[0].children[1], &second[0].children[1]),
        "the other branch is reused"
    );
}

#[test]
fn a_variable_change_recomputes_only_the_nodes_that_read_it() {
    let d = sample();
    let mut resolver = Resolver::new();
    resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(&d, vec![Op::SetVariables {
        set: [(
            c::var_id("surface001"),
            c::variable(json!({"type": "color", "name": "color/surface", "values": [{"value": "#ABCDEF"}]})),
        )].into(),
        remove: Default::default(),
    }]);
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(
        computed, 2,
        "root and leaf A fill with it; nothing else does"
    );
    assert_eq!(
        serde_json::to_value(&tree[0].node.fills()[0]).expect("a paint")["color"],
        json!("#ABCDEF")
    );
}

#[test]
fn a_change_reaches_a_reader_through_an_alias_chain() {
    let d = sample();
    let mut resolver = Resolver::new();
    resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(
        &d,
        vec![Op::SetVariables {
            set: [(
                c::var_id("gap0000001"),
                c::variable(
                    json!({"type": "number", "name": "space/gap", "values": [{"value": 9}]}),
                ),
            )]
            .into(),
            remove: Default::default(),
        }],
    );
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    // `left` reads gap0000002, which aliases gap0000001; leaf C reads gap0000001 directly.
    assert_eq!(computed, 2);
    assert_eq!(
        serde_json::to_value(tree[0].children[0].node.clone()).expect("a node")["gap"],
        json!(9.0)
    );
}

#[test]
fn a_frames_theme_change_recomputes_its_whole_subtree_and_nothing_else() {
    let d = sample();
    let mut resolver = Resolver::new();
    let first = resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(
        &d,
        vec![update("left000001", json!({"theme": {"Mode": "Dark"}}))],
    );
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 3, "left and its two leaves");
    assert_eq!(
        tree[0].children[0].children[0].theme.get("Mode"),
        Some("Dark")
    );
    assert!(Arc::ptr_eq(&first[0].children[1], &tree[0].children[1]));
}

#[test]
fn a_change_to_the_axes_recomputes_everything() {
    let d = sample();
    let mut resolver = Resolver::new();
    resolver.resolve_page(&d, &d.pages[0]);
    let mut shell = Shell::of(&d);
    shell
        .themes
        .insert("Mode".into(), vec!["Dark".into(), "Light".into()]);
    let (next, changes) = apply(
        &d,
        vec![Op::SetShell {
            shell: Box::new(shell),
        }],
    );
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 6);
    assert_eq!(
        tree[0].theme.get("Mode"),
        Some("Dark"),
        "the new first value is the default"
    );
}

#[test]
fn a_change_to_something_the_resolver_does_not_read_keeps_the_cache() {
    let d = sample();
    let mut resolver = Resolver::new();
    resolver.resolve_page(&d, &d.pages[0]);
    let mut shell = Shell::of(&d);
    shell.meta.name = "TEST DATA renamed".into();
    let (next, changes) = apply(
        &d,
        vec![Op::SetShell {
            shell: Box::new(shell),
        }],
    );
    let (_, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 0);
}

#[test]
fn an_inserted_child_rebuilds_its_parent_without_recomputing_the_parent_or_its_siblings() {
    let d = sample();
    let mut resolver = Resolver::new();
    let first = resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(
        &d,
        vec![Op::Insert {
            to: c::in_node("right00001", 1),
            node: Box::new(c::node(rect("leafD00001", json!({})))),
        }],
    );
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 1, "only the new leaf");
    assert_eq!(tree[0].children[1].children.len(), 2);
    assert!(Arc::ptr_eq(
        &first[0].children[1].children[0],
        &tree[0].children[1].children[0]
    ));
    assert!(Arc::ptr_eq(&first[0].children[0], &tree[0].children[0]));
}

#[test]
fn a_removed_subtree_leaves_the_cache() {
    let d = sample();
    let mut resolver = Resolver::new();
    resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(
        &d,
        vec![Op::Delete {
            id: c::id("left000001"),
        }],
    );
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 0);
    assert_eq!(tree[0].children.len(), 1);
    assert_eq!(resolver.cached(), 3, "root, right and leaf C remain");
}

#[test]
fn a_moved_node_takes_the_theme_of_its_new_parent() {
    let mut d = sample();
    let (themed, changes) = apply(
        &d,
        vec![update("left000001", json!({"theme": {"Mode": "Dark"}}))],
    );
    let mut resolver = Resolver::new();
    resolver.invalidate(&themed, &changes);
    d = themed;
    resolver.resolve_page(&d, &d.pages[0]);
    let (next, changes) = apply(
        &d,
        vec![Op::Move {
            id: c::id("leafC00001"),
            to: c::in_node("left000001", 0),
        }],
    );
    let (tree, computed) = step(&mut resolver, &d, &next, &changes);
    assert_eq!(computed, 1);
    assert_eq!(
        tree[0].children[0].children[0].theme.get("Mode"),
        Some("Dark")
    );
}

#[test]
fn a_child_the_dirty_set_never_named_is_still_found() {
    // The parent is not in any change set, and no child list is trusted blindly: the walk finds the new child.
    let d = sample();
    let mut resolver = Resolver::new();
    resolver.resolve_page(&d, &d.pages[0]);
    let (next, _) = apply(
        &d,
        vec![Op::Insert {
            to: c::in_node("right00001", 0),
            node: Box::new(c::node(rect(
                "leafE00001",
                json!({"opacity": var("gap0000001")}),
            ))),
        }],
    );
    let tree = resolver.resolve_page(&next, &next.pages[0]);
    assert_eq!(tree[0].children[1].children.len(), 2);
    assert_eq!(
        tree[0].children[1].children[0].node.id().as_str(),
        "leafE00001"
    );
}
