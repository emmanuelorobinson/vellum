//! Documents, nodes and actors the suites and fakes share. Everything here is `TEST DATA`.

use serde_json::{Value as Json, json};
use vellum_core::ports::{ActorId, Op, Origin, Transaction};
use vellum_types::{Document, Node, NodeId, NodePatch, Page, PageId};

/// A node of `kind` (`"frame"`, `"rect"`, …) with `id`, a name, and a size.
pub fn node(kind: &str, id: &str) -> Node {
    node_with(kind, id, &[], json!({}))
}

/// A node as [`node`], holding `children` (only frames, groups and booleans may), with `extra` fields merged in.
pub fn node_with(kind: &str, id: &str, children: &[Node], extra: Json) -> Node {
    let mut value = json!({
        "type": kind,
        "id": id,
        "name": format!("TEST DATA {id}"),
        "width": 100.0,
        "height": 50.0,
    });
    let map = value.as_object_mut().expect("an object");
    if !children.is_empty() {
        map.insert(
            "children".into(),
            serde_json::to_value(children).expect("nodes serialise"),
        );
    }
    if let Json::Object(extra) = extra {
        map.extend(extra);
    }
    serde_json::from_value(value).expect("a valid node")
}

/// A node id.
pub fn node_id(id: &str) -> NodeId {
    NodeId::new(id).expect("a valid id")
}

/// A page id.
pub fn page_id(id: &str) -> PageId {
    PageId::new(id).expect("a valid id")
}

/// A page with children.
pub fn page(id: &str, children: Vec<Node>) -> Page {
    serde_json::from_value(json!({
        "id": id,
        "name": format!("TEST DATA {id}"),
        "children": serde_json::to_value(children).expect("nodes serialise"),
    }))
    .expect("a valid page")
}

/// Two pages. `page00001` holds a frame `frame00001` with two rectangles (`rect000001`, `rect000002`) and then
/// a loose rectangle `rect000003`; `page00002` is empty. All numbers are on the codec's grid, so the document
/// survives a save unchanged.
pub fn document() -> Document {
    let mut doc = Document::new("TEST DATA: contracts");
    let frame = node_with(
        "frame",
        "frame00001",
        &[node("rect", "rect000001"), node("rect", "rect000002")],
        json!({}),
    );
    doc.pages = vec![
        page("page00001", vec![frame, node("rect", "rect000003")]),
        page("page00002", vec![]),
    ];
    doc
}

/// Alice's id.
pub fn alice() -> ActorId {
    ActorId::new("person:alice")
}

/// Bob's id.
pub fn bob() -> ActorId {
    ActorId::new("person:bob")
}

/// A transaction by `actor` with `ops`.
pub fn tx(actor: &ActorId, ops: Vec<Op>) -> Transaction {
    Transaction {
        origin: Origin::new(actor.clone()),
        ops,
    }
}

/// An op that adds `node` to `page`, inside `parent` or at the top level, at `index`.
pub fn insert_at(page: &str, parent: Option<&str>, index: usize, node: Node) -> Op {
    Op::Insert {
        page: page_id(page),
        parent: parent.map(node_id),
        index,
        node: Box::new(node),
    }
}

/// An op that sets `name` on `id`.
pub fn rename(id: &str, name: &str) -> Op {
    Op::Update {
        id: node_id(id),
        patch: patch(json!({ "name": name })),
    }
}

/// A patch from a JSON object of known node properties.
pub fn patch(properties: Json) -> NodePatch {
    serde_json::from_value(properties).expect("a valid patch")
}

/// The ids of a page's top-level nodes.
pub fn top_ids(doc: &Document, page: &str) -> Vec<String> {
    doc.pages
        .iter()
        .find(|p| p.id.as_str() == page)
        .map(|p| p.children.iter().map(|n| n.id().to_string()).collect())
        .unwrap_or_default()
}

/// Every node id in the document, depth first.
pub fn all_ids(doc: &Document) -> Vec<String> {
    fn walk(node: &Node, out: &mut Vec<String>) {
        out.push(node.id().to_string());
        node.children().iter().for_each(|c| walk(c, out));
    }
    let mut out = Vec::new();
    for page in &doc.pages {
        page.children.iter().for_each(|n| walk(n, &mut out));
    }
    out
}

/// Finds a node by id.
pub fn find<'a>(doc: &'a Document, id: &str) -> Option<&'a Node> {
    fn walk<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
        if node.id().as_str() == id {
            return Some(node);
        }
        node.children().iter().find_map(|c| walk(c, id))
    }
    doc.pages
        .iter()
        .flat_map(|p| &p.children)
        .find_map(|n| walk(n, id))
}
