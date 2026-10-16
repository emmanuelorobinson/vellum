//! What every mutating tool shares: one transaction per call, errors reported where the agent wrote the mistake,
//! and new nodes read from the agent's JSON (ARCHITECTURE §7.3: "A bad op rejects the batch with a precise error
//! the agent can act on").
//!
//! A tool turns its input into ops, each with a [`Label`]: which input item it came from and how the op's fields
//! are spelled in the input. When the document refuses an op, the core's error (`ops[2].to.parent`) is turned back
//! into the input's spelling (`moves[2].parent`), with the item's index and the reason in plain words.

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use vellum_core::guard::WriteError;
use vellum_core::ops::{Applied, Op, OpError, Parent, Place, Transaction};
use vellum_core::ports::{IdGen, TargetError};
use vellum_types::{Document, Node, NodeId};

use super::step::{Counts, StepNote};
use super::{ToolCtx, ToolError};

/// Where an op came from in the tool's input.
pub(super) struct Label {
    /// The input item, counting from 0: what the agent is told to fix. None when the op stands for the whole
    /// input rather than one item of a list.
    item: Option<usize>,
    /// The input path of the item, such as `updates[1]`.
    prefix: String,
    /// The op's field paths that the input spells differently, and the input path for each.
    renames: Vec<(String, String)>,
}

impl Label {
    /// An op made from input item `item` at `prefix`.
    pub(super) fn new(item: usize, prefix: impl Into<String>) -> Self {
        Self {
            item: Some(item),
            prefix: prefix.into(),
            renames: Vec::new(),
        }
    }

    /// An op made from the input as a whole, whose fields are at `prefix`.
    pub(super) fn whole(prefix: impl Into<String>) -> Self {
        Self {
            item: None,
            prefix: prefix.into(),
            renames: Vec::new(),
        }
    }

    /// The op's field `from` (and what is inside it) is the input's `to`.
    pub(super) fn rename(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.renames.push((from.into(), to.into()));
        self
    }

    /// The input path for the op's field path `path`.
    fn locate(&self, path: &str) -> String {
        for (from, to) in &self.renames {
            if let Some(rest) = path.strip_prefix(from.as_str())
                && (rest.is_empty() || rest.starts_with('.') || rest.starts_with('['))
            {
                return join(to, rest.strip_prefix('.').unwrap_or(rest));
            }
        }
        join(&self.prefix, path)
    }
}

/// `a.b`, `a[1]`, or whichever of them is not empty.
pub(super) fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (_, true) => a.to_owned(),
        (true, false) => b.to_owned(),
        (false, false) if b.starts_with('[') => format!("{a}{b}"),
        (false, false) => format!("{a}.{b}"),
    }
}

/// The ops of one call, each with where it came from.
#[derive(Default)]
pub(super) struct Batch {
    ops: Vec<Op>,
    labels: Vec<Label>,
}

impl Batch {
    pub(super) fn push(&mut self, op: Op, label: Label) {
        self.ops.push(op);
        self.labels.push(label);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// What a committed batch did: the change set and the ids of copies.
pub(super) struct Committed {
    pub applied: Applied,
}

/// The first letter of `text` in capitals: the core writes reasons to follow "Change 2 was refused: ".
pub(super) fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn batch_error(labels: &[Label], error: &OpError) -> ToolError {
    let reason = sentence(&error.kind.to_string());
    match labels.get(error.op_index) {
        Some(label) => {
            let found = ToolError::invalid(label.locate(&error.path), reason);
            match label.item {
                Some(item) => found.at_op(item),
                None => found,
            }
        }
        None => ToolError::invalid("", reason),
    }
}

impl ToolCtx<'_> {
    /// Applies `batch` to the document at `path` as one transaction by this connection's agent, through the
    /// Guard, or refuses it whole. Notes the step's counts and target.
    pub(super) fn commit(&mut self, path: &str, batch: Batch) -> Result<Committed, ToolError> {
        if batch.is_empty() {
            return Err(ToolError::invalid("", "There is nothing to change."));
        }
        let before = self.snapshot(path)?;
        let origin = self.origin();
        let Batch { ops, labels } = batch;
        let target = self.target(path)?;
        let applied = match target.transact(Transaction::new(origin, ops)) {
            Ok(applied) => applied,
            Err(TargetError::Write(WriteError::Invalid(error))) => {
                return Err(batch_error(&labels, &error));
            }
            Err(other) => return Err(other.into()),
        };
        let after = target.snapshot()?;
        self.noted(path, &before, &after, &applied.changes);
        Ok(Committed { applied })
    }

    /// Notes what a change did for the step: its counts and the frame it was in.
    pub(super) fn noted(
        &mut self,
        path: &str,
        before: &Document,
        after: &Document,
        changes: &vellum_core::ports::ChangeSet,
    ) {
        self.note.document = Some(path.to_owned());
        self.note.counts = Counts::of(changes);
        if self.note.target.is_none() {
            self.note.target = StepNote::frame_of(before, after, changes);
        }
    }

    /// Notes the node a step is about, when it is not simply where the changes were.
    pub(super) fn note_target(&mut self, doc: &Document, id: &str) {
        self.note.target = StepNote::node(doc, id);
    }

    /// Notes what an `execute` step ran.
    pub(super) fn note_script(&mut self, code: &str) {
        self.note.script = Some(code.to_owned());
    }

    /// Notes what a progress report said.
    pub(super) fn note_progress(&mut self, message: &str, progress: Option<(u32, u32)>) {
        self.note.message = Some(message.to_owned());
        self.note.progress = progress;
    }
}

/// Where a node goes: a page or a node, by id, and an index (the end when there is none). A parent that does not
/// exist is left for the transaction to refuse, so the error says so in the same words as any other.
pub(super) fn place(
    doc: &Document,
    parent: &str,
    index: Option<u32>,
    moving: Option<&str>,
    at: &str,
) -> Result<Place, ToolError> {
    let (parent_ref, children): (Parent, &[Node]) =
        if let Some(page) = doc.pages.iter().find(|p| p.id.as_str() == parent) {
            (Parent::Page(page.id.clone()), &page.children)
        } else {
            let id = NodeId::new(parent).map_err(|e| {
                ToolError::invalid(at, format!("{parent:?} is not a page or layer id: {e}."))
            })?;
            let children = vellum_core::ops::find(doc, &id).map_or(&[][..], Node::children);
            (Parent::Node(id), children)
        };
    let len = children.len()
        - usize::from(moving.is_some_and(|m| children.iter().any(|n| n.id().as_str() == m)));
    Ok(Place {
        parent: parent_ref,
        index: index.map_or(len, |i| i as usize),
    })
}

/// A node id the agent gave, checked.
pub(super) fn node_id(id: &str, at: &str) -> Result<NodeId, ToolError> {
    NodeId::new(id).map_err(|e| ToolError::invalid(at, format!("{id:?} is not a layer id: {e}.")))
}

/// Every id a document uses, so new ones never repeat one.
pub(super) fn ids_in_use(doc: &Document) -> BTreeSet<String> {
    fn walk(nodes: &[Node], out: &mut BTreeSet<String>) {
        for node in nodes {
            out.insert(node.id().as_str().to_owned());
            walk(node.children(), out);
        }
    }
    let mut out = BTreeSet::new();
    for page in &doc.pages {
        out.insert(page.id.as_str().to_owned());
        walk(&page.children, &mut out);
    }
    for (id, component) in &doc.components {
        out.insert(id.as_str().to_owned());
        walk(std::slice::from_ref(&component.root), &mut out);
    }
    out.extend(doc.variables.keys().map(|id| id.as_str().to_owned()));
    out
}

/// A fresh id from `ids`, one nothing in `taken` uses, which it then is.
pub(super) fn fresh(ids: &mut dyn IdGen, taken: &mut BTreeSet<String>) -> String {
    loop {
        let id = ids.node_id().as_str().to_owned();
        if taken.insert(id.clone()) {
            return id;
        }
    }
}

/// The deepest new node tree a tool reads.
const MAX_DEPTH: usize = 64;

/// How a new node without a name is called in the layers panel: its text, else its kind.
fn default_name(node: &Map<String, Value>) -> String {
    let kind = node.get("type").and_then(Value::as_str).unwrap_or("layer");
    let text: Option<String> = match node.get("content") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(runs)) => Some(
            runs.iter()
                .filter_map(|r| r.get("text").and_then(Value::as_str))
                .collect(),
        ),
        _ => None,
    };
    if let Some(text) = text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()) {
        return text.chars().take(40).collect();
    }
    let mut chars = kind.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Gives every node in `value` an id (when it has none) and a name, and an instance of a local component the
/// component's size and name when it does not give its own.
fn complete(
    value: &mut Value,
    doc: &Document,
    ids: &mut dyn IdGen,
    taken: &mut BTreeSet<String>,
    at: &str,
    depth: usize,
) -> Result<(), ToolError> {
    if depth > MAX_DEPTH {
        return Err(ToolError::invalid(
            at,
            format!("Nodes nest at most {MAX_DEPTH} deep."),
        ));
    }
    let Value::Object(node) = value else {
        return Err(ToolError::invalid(at, "A node is an object with a `type`."));
    };
    if !node.contains_key("type") {
        return Err(ToolError::invalid(
            at,
            "A node needs a `type`, such as \"frame\", \"text\" or \"rect\".",
        ));
    }
    match node.get("id") {
        None => {
            node.insert("id".into(), json!(fresh(ids, taken)));
        }
        Some(Value::String(id)) => {
            taken.insert(id.clone());
        }
        Some(_) => return Err(ToolError::invalid(join(at, "id"), "An id is a string.")),
    }
    if node.get("type") == Some(&json!("instance"))
        && !node.contains_key("library")
        && let Some(component) = node
            .get("componentId")
            .and_then(Value::as_str)
            .and_then(|id| doc.components.iter().find(|(c, _)| c.as_str() == id))
    {
        let root = &component.1.root;
        node.entry("width")
            .or_insert_with(|| serde_json::to_value(root.width()).expect("a size"));
        node.entry("height")
            .or_insert_with(|| serde_json::to_value(root.height()).expect("a size"));
        node.entry("name").or_insert_with(|| json!(root.name()));
    }
    if !node.contains_key("name") {
        let name = default_name(node);
        node.insert("name".into(), json!(name));
    }
    if let Some(children) = node.get_mut("children") {
        let Value::Array(children) = children else {
            return Err(ToolError::invalid(
                join(at, "children"),
                "children is a list of nodes.",
            ));
        };
        for (i, child) in children.iter_mut().enumerate() {
            complete(
                child,
                doc,
                ids,
                taken,
                &format!("{}[{i}]", join(at, "children")),
                depth + 1,
            )?;
        }
    }
    Ok(())
}

/// The field a serde message names, as in "missing field `width`" or "unknown field `colour`".
fn named_field(message: &str) -> Option<String> {
    for marker in ["missing field `", "unknown field `", "duplicate field `"] {
        if let Some(rest) = message.split(marker).nth(1) {
            return rest.split('`').next().map(str::to_owned);
        }
    }
    None
}

/// Where in `value` a node fails to read, and why. Nodes are read by their `type` tag, which hides the failing
/// field from serde, so it is found by looking at the children first and then at each property in turn.
fn locate_node_error(value: &Value) -> Option<(String, String)> {
    let error = serde_json::from_value::<Node>(value.clone()).err()?;
    let object = value.as_object()?;
    if let Some(Value::Array(children)) = object.get("children") {
        for (i, child) in children.iter().enumerate() {
            if let Some((path, message)) = locate_node_error(child) {
                return Some((join(&format!("children[{i}]"), &path), message));
            }
        }
    }
    let mut alone = object.clone();
    alone.remove("children");
    let error = match serde_json::from_value::<Node>(Value::Object(alone.clone())) {
        Ok(_) => error,
        Err(e) => e,
    };
    let message = error.to_string();
    if let Some(field) = named_field(&message) {
        return Some((field, message));
    }
    for key in alone.keys().filter(|k| *k != "type") {
        let mut without = alone.clone();
        without.remove(key);
        match serde_json::from_value::<Node>(Value::Object(without)) {
            Ok(_) => return Some((key.clone(), message)),
            Err(e) if named_field(&e.to_string()).as_deref() == Some(key) => {
                return Some((key.clone(), message));
            }
            Err(_) => {}
        }
    }
    Some((String::new(), message))
}

/// Reads a new node from the agent's JSON at input path `at`: ids and names filled in, then checked as the file
/// format reads it. `taken` holds every id in use and gains the new ones.
pub(super) fn new_node(
    mut value: Value,
    doc: &Document,
    ids: &mut dyn IdGen,
    taken: &mut BTreeSet<String>,
    at: &str,
) -> Result<Node, ToolError> {
    complete(&mut value, doc, ids, taken, at, 0)?;
    serde_json::from_value::<Node>(value.clone()).map_err(|_| {
        let (path, message) =
            locate_node_error(&value).unwrap_or_else(|| (String::new(), "Not a node.".into()));
        ToolError::invalid(join(at, &path), sentence(&message))
    })
}

/// How many nodes a tree holds.
pub(super) fn count(node: &Node) -> usize {
    1 + node.children().iter().map(count).sum::<usize>()
}
