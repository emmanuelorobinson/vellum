use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use vellum_core::{
    ports::*,
    resolver::{
        ResolvedNode,
        components::Resolver,
        libraries::{Libraries, LibrarySource},
    },
};
use vellum_types::{Document, Node, NodeIdPath};
pub fn id(s: &str) -> NodeIdPath {
    s.parse().unwrap()
}
pub fn node(s: &str, kind: &str, fields: Value) -> Value {
    let mut n = json!({"id":s,"type":kind,"name":"TEST DATA","width":100,"height":100});
    n.as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    n
}
struct NoLibraries;
impl LibrarySource for NoLibraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}
pub fn resolve(nodes: Vec<Value>) -> Vec<Arc<ResolvedNode>> {
    resolve_in(&Document::new("TEST DATA"), nodes)
}
pub fn resolve_in(doc: &Document, nodes: Vec<Value>) -> Vec<Arc<ResolvedNode>> {
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect();
    Resolver::new().resolve_nodes(
        doc,
        "test.vellum",
        &nodes,
        &mut Libraries::new(&NoLibraries),
    )
}
pub fn boxes(entries: &[(&str, [f64; 4])]) -> LayoutResult {
    LayoutResult {
        boxes: entries
            .iter()
            .map(|(s, b)| (id(s), Rect::new(b[0], b[1], b[2], b[3])))
            .collect::<BTreeMap<_, _>>(),
        problems: vec![],
    }
}
