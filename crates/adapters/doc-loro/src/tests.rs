//! What only the adapter can see: how our ids map onto Loro's tree.

use vellum_contracts::fakes::SequentialIds;
use vellum_contracts::samples::{alice, document, insert_at, node, node_id, node_with, tx};
use vellum_core::ops::{self, Op};
use vellum_core::ports::DocStore;

use crate::LoroDocStore;

fn mapping_holds(store: &LoroDocStore) {
    let doc = store.snapshot();
    let mut expected: Vec<String> = Vec::new();
    let mut stack: Vec<_> = doc.pages.iter().flat_map(|p| &p.children).collect();
    stack.extend(doc.components.values().map(|c| &c.root));
    while let Some(node) = stack.pop() {
        expected.push(node.id().to_string());
        stack.extend(node.children());
    }
    expected.sort();
    let mapped = store.backend.mapped();
    let ours: Vec<String> = mapped.iter().map(|(id, _)| id.to_string()).collect();
    assert_eq!(ours, expected, "every node maps to one tree node");
    for (id, written) in &mapped {
        assert_eq!(
            written,
            &format!("\"{id}\""),
            "the tree node for {id} names another node"
        );
    }
    assert_eq!(
        store.backend.tree_len(),
        expected.len() + doc.pages.len() + doc.components.len(),
        "the tree holds one node per page, per component and per document node, nothing else"
    );
    assert_eq!(store.read_back(), doc, "Loro reads back as the document");
}

#[test]
fn our_node_ids_map_to_loro_tree_nodes() {
    let mut store = LoroDocStore::new(&document(), 1);
    mapping_holds(&store);
    let mut ids = SequentialIds::new();
    let group = node_with(
        "group",
        "group00001",
        &[node("rect", "kid0000001")],
        serde_json::json!({}),
    );
    ops::apply(
        &mut store,
        &mut ids,
        tx(&alice(), vec![insert_at("page00002", None, 0, group)]),
    )
    .expect("insert");
    mapping_holds(&store);
    ops::apply(
        &mut store,
        &mut ids,
        tx(
            &alice(),
            vec![Op::Delete {
                id: node_id("frame00001"),
            }],
        ),
    )
    .expect("delete");
    mapping_holds(&store);
    store.undo(&alice()).expect("undo the delete");
    mapping_holds(&store);
    store.undo(&alice()).expect("undo the insert");
    mapping_holds(&store);
    ops::apply(
        &mut store,
        &mut ids,
        tx(
            &alice(),
            vec![Op::CreateComponent {
                id: node_id("frame00001"),
                component_id: vellum_types::ComponentId::new("component1").expect("an id"),
            }],
        ),
    )
    .expect("make a component");
    mapping_holds(&store);
}
