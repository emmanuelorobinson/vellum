//! The document, ids and store the op tests share. Everything here is `TEST DATA`.

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};
use vellum_types::{
    Actor, DeviceId, Document, Node, NodeId, NodePatch, Origin, PageId, Variable, VariableId,
};

use crate::ops::{Op, OpError, Parent, Place, Plan, Transaction, apply_edits, plan};
use crate::ports::{ChangeSet, Commit, DocStore, IdGen, SyncCursor, SyncError, SyncPayload};

pub const ALICE: &str = "01JA0000000000000000000001";
pub const BOB: &str = "01JA0000000000000000000002";

pub fn person(device: &str) -> Actor {
    Actor::Person {
        device: DeviceId::new(device).expect("a ULID"),
    }
}

pub fn alice() -> Origin {
    Origin::new(person(ALICE))
}

pub fn tx(ops: Vec<Op>) -> Transaction {
    Transaction::new(alice(), ops)
}

pub fn id(text: &str) -> NodeId {
    NodeId::new(text).expect("a valid id")
}

pub fn page_id(text: &str) -> PageId {
    PageId::new(text).expect("a valid id")
}

pub fn var_id(text: &str) -> VariableId {
    VariableId::new(text).expect("a valid id")
}

pub fn node(value: Json) -> Node {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{value}: {e}"))
}

/// A node of `kind` with `id`, a fixed 10 x 10 size and `extra` fields merged in.
pub fn shape(kind: &str, node_id: &str, extra: Json) -> Node {
    let mut value = json!({"type": kind, "id": node_id, "name": format!("TEST DATA {node_id}"),
                           "width": 10, "height": 10});
    if let (Json::Object(map), Json::Object(extra)) = (&mut value, extra) {
        map.extend(extra);
    }
    node(value)
}

pub fn at(x: f64, y: f64) -> Json {
    json!({"position": {"type": "absolute", "x": x, "y": y}})
}

pub fn variable(value: Json) -> Variable {
    serde_json::from_value(value).expect("a valid variable")
}

pub fn patch(value: Json) -> NodePatch {
    let Json::Object(map) = value else {
        panic!("a patch is an object")
    };
    NodePatch::new(map.into_iter().collect::<BTreeMap<_, _>>()).expect("known properties")
}

pub fn on_page(page: &str, index: usize) -> Place {
    Place {
        parent: Parent::Page(page_id(page)),
        index,
    }
}

pub fn in_node(parent: &str, index: usize) -> Place {
    Place {
        parent: Parent::Node(id(parent)),
        index,
    }
}

/// `canvas0001` (canvas): `frame00001` at (100, 50) holding `rect000001` at (10, 10) and `rect000002` at
/// (30, 30); `rect000003`; `group00001` at (200, 200) holding `ellipse001` at (5, 5); `bool000001` holding
/// `rect000004`; `text000001`, whose fill uses `color00001`.
/// `slides0001` (slides): `slide00001`, a frame.
/// Variables `color00001` (colour) and `number0001` (number). Component `comp000001`, root `croot00001` holding
/// `ctext00001`.
pub fn document() -> Document {
    serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA ops", "createdWith": "tests"},
        "variables": {
            "color00001": {"type": "color", "name": "color/primary", "values": [{"value": "#3366FF"}]},
            "number0001": {"type": "number", "name": "space/md", "values": [{"value": 16}]}
        },
        "components": {
            "comp000001": {"name": "Button", "root": {
                "type": "frame", "id": "croot00001", "name": "Button", "width": "hug", "height": "hug",
                "children": [{"type": "text", "id": "ctext00001", "name": "Label", "width": "hug",
                              "height": "hug", "content": [{"text": "OK", "font": "Inter", "size": 14}]}]
            }}
        },
        "pages": [
            {"id": "canvas0001", "name": "Canvas", "children": [
                {"type": "frame", "id": "frame00001", "name": "Card", "width": 300, "height": 200,
                 "position": {"type": "absolute", "x": 100, "y": 50},
                 "children": [
                    {"type": "rect", "id": "rect000001", "name": "A", "width": 10, "height": 10,
                     "position": {"type": "absolute", "x": 10, "y": 10}},
                    {"type": "rect", "id": "rect000002", "name": "B", "width": 10, "height": 10,
                     "position": {"type": "absolute", "x": 30, "y": 30}}
                 ]},
                {"type": "rect", "id": "rect000003", "name": "C", "width": 10, "height": 10},
                {"type": "group", "id": "group00001", "name": "G", "width": "hug", "height": "hug",
                 "position": {"type": "absolute", "x": 200, "y": 200},
                 "children": [{"type": "ellipse", "id": "ellipse001", "name": "E", "width": 10, "height": 10,
                               "position": {"type": "absolute", "x": 5, "y": 5}}]},
                {"type": "boolean", "id": "bool000001", "name": "Bool", "op": "union", "width": "hug", "height": "hug",
                 "children": [{"type": "rect", "id": "rect000004", "name": "D", "width": 10, "height": 10}]},
                {"type": "text", "id": "text000001", "name": "T", "width": "hug", "height": "hug",
                 "content": [{"text": "Hi", "font": "Inter", "size": 14}], "fills": [{"type": "solid", "color": {"$var": "color00001"}}]}
            ]},
            {"id": "slides0001", "name": "Slides", "kind": "slides", "children": [
                {"type": "frame", "id": "slide00001", "name": "Slide 1", "width": 1920, "height": 1080}
            ]}
        ]
    }))
    .expect("the sample document is valid")
}

/// Ids `new0000001`, `new0000002`, …
#[derive(Debug, Default)]
pub struct Ids(u32);

impl IdGen for Ids {
    fn node_id(&mut self) -> NodeId {
        self.0 += 1;
        id(&format!("new{:07}", self.0))
    }
}

pub fn planned(doc: &Document, ops: Vec<Op>) -> Result<Plan, OpError> {
    plan(doc, &tx(ops), &mut Ids::default())
}

/// Plans `ops` on the sample document and returns the result; panics if refused.
pub fn accepted(ops: Vec<Op>) -> Document {
    planned(&document(), ops)
        .unwrap_or_else(|e| panic!("refused: {e} at {}", e.location()))
        .document
}

/// Plans `ops` on the sample document and returns the error; panics if accepted.
pub fn refused(ops: Vec<Op>) -> OpError {
    match planned(&document(), ops) {
        Ok(_) => panic!("the transaction should have been refused"),
        Err(error) => error,
    }
}

pub fn find<'a>(doc: &'a Document, node_id: &str) -> Option<&'a Node> {
    crate::ops::find(doc, &id(node_id))
}

pub fn child_ids(doc: &Document, parent: &str) -> Vec<String> {
    let children = match doc.pages.iter().find(|p| p.id.as_str() == parent) {
        Some(page) => &page.children,
        None => find(doc, parent).expect("the parent").children(),
    };
    children.iter().map(|n| n.id().to_string()).collect()
}

pub fn position(doc: &Document, node_id: &str) -> Json {
    serde_json::to_value(find(doc, node_id).expect("the node").position()).expect("serialise")
}

/// A store that keeps the document and each commit's inverse, enough to exercise `ops::apply`.
#[derive(Debug)]
pub struct Store {
    pub doc: Document,
    pub undo: Vec<(Actor, Vec<crate::ops::Edit>)>,
    pub applied: usize,
}

impl Store {
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            undo: Vec::new(),
            applied: 0,
        }
    }
}

impl DocStore for Store {
    fn adapter_id(&self) -> &'static str {
        "ops-tests"
    }

    fn snapshot(&self) -> Document {
        self.doc.clone()
    }

    fn apply(&mut self, commit: Commit) -> ChangeSet {
        let (origin, edits, inverse, changes) = commit.into_parts();
        apply_edits(&mut self.doc, &edits, origin.clone()).expect("a commit applies");
        self.undo.push((origin.actor, inverse));
        self.applied += 1;
        changes
    }

    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let at = self.undo.iter().rposition(|(a, _)| a == actor)?;
        let (_, inverse) = self.undo.remove(at);
        let (_, changes) = apply_edits(&mut self.doc, &inverse, Origin::new(actor.clone())).ok()?;
        Some(changes)
    }

    fn redo(&mut self, _actor: &Actor) -> Option<ChangeSet> {
        None
    }

    fn cursor(&self) -> SyncCursor {
        SyncCursor::empty()
    }

    fn export_updates(&self, _since: &SyncCursor) -> SyncPayload {
        SyncPayload {
            adapter: self.adapter_id().into(),
            bytes: Vec::new(),
        }
    }

    fn import_updates(&mut self, _payload: &SyncPayload) -> Result<ChangeSet, SyncError> {
        Err(SyncError::Malformed("the op tests do not sync".into()))
    }
}
