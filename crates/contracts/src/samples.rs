//! Documents, nodes and actors the suites and fakes share. Everything here is `TEST DATA`.

use serde_json::{Value as Json, json};
use vellum_core::ops::{Op, Parent, Place, Transaction};
use vellum_types::{
    Actor, AgentId, DeviceId, Document, Node, NodeId, NodePatch, Origin, Page, PageId,
};

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

/// Alice, a person at her device.
pub fn alice() -> Actor {
    Actor::Person {
        device: DeviceId::new("01JA0000000000000000000A11").expect("a ULID"),
    }
}

/// Bob, a person at his device.
pub fn bob() -> Actor {
    Actor::Person {
        device: DeviceId::new("01JA0000000000000000000B0B").expect("a ULID"),
    }
}

/// Teal, a built-in agent.
pub fn teal() -> AgentId {
    AgentId::new("01JA00000000000000000TEA10").expect("a ULID")
}

/// Rose, another built-in agent.
pub fn rose() -> AgentId {
    AgentId::new("01JA00000000000000000R0SE0").expect("a ULID")
}

/// A transaction by `actor` with `ops`.
pub fn tx(actor: &Actor, ops: Vec<Op>) -> Transaction {
    Transaction::new(Origin::new(actor.clone()), ops)
}

/// An op that adds `node` inside `parent`, or at the top level of `page` when there is none, at `index`.
pub fn insert_at(page: &str, parent: Option<&str>, index: usize, node: Node) -> Op {
    let parent = match parent {
        Some(parent) => Parent::Node(node_id(parent)),
        None => Parent::Page(page_id(page)),
    };
    Op::Insert {
        to: Place { parent, index },
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

/// A token set every format can hold in full: each type, an alias, a name that needs a hint, a string that looks
/// like a colour, and values under non-default theme values on two axes.
pub fn token_set() -> vellum_core::tokens::TokenSet {
    use vellum_core::tokens::{ModeValue, Token, TokenSet, TokenValue};
    use vellum_types::{Color, VariableType};
    let token = |name: &str, kind, value, modes| Token {
        name: name.into(),
        kind,
        value,
        modes,
    };
    let mode = |axis: &str, mode: &str, value| ModeValue {
        axis: axis.into(),
        mode: mode.into(),
        value,
    };
    TokenSet {
        themes: [
            (
                "Device".to_owned(),
                vec!["Desktop".to_owned(), "Phone".to_owned()],
            ),
            (
                "Mode".to_owned(),
                vec!["Light".to_owned(), "Dark".to_owned()],
            ),
        ]
        .into(),
        tokens: vec![
            token(
                "color/accent",
                VariableType::Color,
                TokenValue::Alias("color/brand/primary".into()),
                vec![],
            ),
            token(
                "color/brand/primary",
                VariableType::Color,
                TokenValue::Color(Color::rgb(0x25, 0x63, 0xEB)),
                vec![mode(
                    "Mode",
                    "Dark",
                    TokenValue::Color(Color::rgba(0x60, 0xA5, 0xFA, 0xCC)),
                )],
            ),
            token(
                "flag/dense",
                VariableType::Boolean,
                TokenValue::Boolean(false),
                vec![mode("Device", "Phone", TokenValue::Boolean(true))],
            ),
            token(
                "font/body",
                VariableType::String,
                TokenValue::String("Inter, sans-serif".into()),
                vec![],
            ),
            token(
                "label/hex",
                VariableType::String,
                TokenValue::String("#2563eb".into()),
                vec![],
            ),
            token(
                "radius/md",
                VariableType::Number,
                TokenValue::Number(8.0),
                vec![],
            ),
            token(
                "space/4",
                VariableType::Number,
                TokenValue::Number(16.0),
                vec![mode("Device", "Phone", TokenValue::Number(12.5))],
            ),
        ],
    }
}
