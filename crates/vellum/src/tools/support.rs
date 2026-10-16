//! What every tool needs: reading its input, finding the document it names, and walking a document's nodes.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Component as PathPart, Path, PathBuf};

use schemars::generate::SchemaSettings;
use schemars::transform::{Transform, transform_subschemas};
use schemars::{JsonSchema, Schema};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use vellum_core::ports::DocumentTarget;
use vellum_types::{Component, ComponentId, Document, Node, NodeId, Page};

use super::{ToolCtx, ToolError};

/// Reads a tool's input. `null` counts as an empty object; a wrong field is reported with its name.
pub fn parse_input<T: DeserializeOwned>(input: Value) -> Result<T, ToolError> {
    let input = if input.is_null() { json!({}) } else { input };
    serde_path_to_error::deserialize(input).map_err(|error| {
        let path = error.path().to_string();
        let path = if path == "." { String::new() } else { path };
        ToolError::invalid(path, error.inner().to_string())
    })
}

/// The input schema of a tool whose input is `T`. An optional field is simply not required (no `null`
/// alternative), and shared types are written out where they are used, so no client has to chase a `$ref`.
pub(crate) fn schema_of<T: JsonSchema>() -> Schema {
    let settings = SchemaSettings::draft2020_12().with(|s| {
        s.inline_subschemas = true;
        s.transforms.push(Box::new(NoNull));
    });
    settings.into_generator().into_root_schema_for::<T>()
}

/// Writes an optional field as the type it has when present: `"type": "string"`, not `["string", "null"]`.
#[derive(Clone)]
struct NoNull;

impl Transform for NoNull {
    fn transform(&mut self, schema: &mut Schema) {
        transform_subschemas(self, schema);
        let Some(object) = schema.as_object_mut() else {
            return;
        };
        if let Some(Value::Array(types)) = object.get_mut("type") {
            types.retain(|t| t != "null");
            if types.len() == 1 {
                let only = types.remove(0);
                object.insert("type".into(), only);
            }
        }
        if let Some(Value::Array(words)) = object.get_mut("enum") {
            words.retain(|w| !w.is_null());
        }
        if let Some(Value::Array(options)) = object.get_mut("anyOf") {
            options.retain(|o| o.get("type") != Some(&Value::String("null".into())));
            if options.len() == 1
                && let Value::Object(only) = options.remove(0)
            {
                object.remove("anyOf");
                for (key, value) in only {
                    object.entry(key).or_insert(value);
                }
            }
        }
    }
}

impl ToolCtx<'_> {
    /// The key a document path goes by. With a known working directory that is the absolute path inside it, so
    /// two clients naming the same file reach the same document; without one the path is used as given.
    pub(super) fn resolve(&self, path: &str) -> Result<String, ToolError> {
        if Path::new(path)
            .extension()
            .is_none_or(|ext| ext != "vellum")
        {
            return Err(ToolError::invalid(
                "document",
                format!(
                    "{path} is not a document: documents are .vellum files, or folders ending in .vellum."
                ),
            ));
        }
        let Some(cwd) = &self.session.cwd else {
            return Ok(path.to_owned());
        };
        let wanted = Path::new(path);
        let joined = if wanted.is_absolute() {
            wanted.to_path_buf()
        } else {
            cwd.join(wanted)
        };
        let mut clean = PathBuf::new();
        for part in joined.components() {
            match part {
                PathPart::ParentDir => {
                    clean.pop();
                }
                PathPart::CurDir => {}
                other => clean.push(other.as_os_str()),
            }
        }
        if !clean.starts_with(cwd) {
            return Err(ToolError::invalid(
                "document",
                format!(
                    "{path} is outside {}. A document has to be inside the directory the client was started in.",
                    cwd.display()
                ),
            ));
        }
        Ok(clean.to_string_lossy().into_owned())
    }

    /// Whether this connection may see the document at `path`: always, when its working directory is not known;
    /// otherwise only inside it. Another client's documents are in the daemon but not in this client's view.
    pub(super) fn inside(&self, path: &str) -> bool {
        self.session
            .cwd
            .as_ref()
            .is_none_or(|cwd| Path::new(path).starts_with(cwd))
    }

    /// The open documents this connection may see, sorted.
    pub(super) fn visible_paths(&mut self) -> Vec<String> {
        let paths = self.docs.paths();
        paths.into_iter().filter(|path| self.inside(path)).collect()
    }

    /// The document this connection works on when a call does not name one: the one opened or used last, else
    /// the only one open, among those it may see.
    pub(super) fn default_document(&mut self) -> Option<String> {
        if let Some(active) = self.docs.active()
            && self.inside(active)
        {
            return Some(active.to_owned());
        }
        match self.visible_paths().as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        }
    }

    /// The document a call is about: the one it names, else the default. Naming a document makes it the one
    /// later calls use.
    pub(super) fn document_path(&mut self, named: Option<&str>) -> Result<String, ToolError> {
        if let Some(path) = named {
            return self.resolve(path);
        }
        if let Some(path) = self.default_document() {
            return Ok(path);
        }
        Err(ToolError::invalid(
            "document",
            if self.visible_paths().is_empty() {
                "No document is open. Call open_document with the path of a .vellum file, or pass `document`."
            } else {
                "More than one document is open. Pass `document` with the path of the one you mean."
            },
        ))
    }

    /// The target for `path`, opening the file headless when nobody holds it. The document becomes the active one.
    pub(super) fn target(&mut self, path: &str) -> Result<&mut dyn DocumentTarget, ToolError> {
        if !self.inside(path) {
            return Err(ToolError::invalid(
                "document",
                format!("{path} is outside the directory this client was started in."),
            ));
        }
        // Checked first so the message can name the path and how to make the file.
        if !self.docs.exists(path)? {
            return Err(ToolError::not_found(format!(
                "There is no document at {path}. Pass `create: true` to open_document to make one."
            )));
        }
        self.docs.set_active(path);
        Ok(self.docs.target(path)?)
    }

    /// The document at `path` as its holder has it, unsaved changes included. The first document a call reads
    /// is the one its step names.
    pub(super) fn snapshot(&mut self, path: &str) -> Result<Document, ToolError> {
        let doc = self.target(path)?.snapshot()?;
        self.note.document.get_or_insert_with(|| path.to_owned());
        Ok(doc)
    }

    /// The path of the document `named` (or the active one) and its content.
    pub(super) fn document(
        &mut self,
        named: Option<&str>,
    ) -> Result<(String, Document), ToolError> {
        let path = self.document_path(named)?;
        let doc = self.snapshot(&path)?;
        Ok((path, doc))
    }
}

/// One node in a walk of the document, with where it sits.
pub struct Entry<'a> {
    /// The node.
    pub node: &'a Node,
    /// Its parent, none at the top of a page or a component.
    pub parent: Option<&'a NodeId>,
    /// The page it is on.
    pub page: Option<&'a Page>,
    /// The component whose tree it is in.
    pub component: Option<(&'a ComponentId, &'a Component)>,
    /// One past the last entry of its subtree: the subtree is `entries[i..end]`.
    pub end: usize,
}

/// Every node of a document in reading order: pages in order, then components by id, each tree parent first.
pub struct Index<'a> {
    /// The nodes.
    pub entries: Vec<Entry<'a>>,
    by_id: HashMap<&'a str, usize>,
    pages: HashMap<&'a str, Range<usize>>,
    components: HashMap<&'a str, Range<usize>>,
}

impl<'a> Index<'a> {
    /// Walks `doc`.
    pub fn build(doc: &'a Document) -> Self {
        let mut index = Self {
            entries: Vec::new(),
            by_id: HashMap::new(),
            pages: HashMap::new(),
            components: HashMap::new(),
        };
        for page in &doc.pages {
            let start = index.entries.len();
            for node in &page.children {
                index.push(node, None, Some(page), None);
            }
            index
                .pages
                .insert(page.id.as_str(), start..index.entries.len());
        }
        for (id, component) in &doc.components {
            let start = index.entries.len();
            index.push(&component.root, None, None, Some((id, component)));
            index
                .components
                .insert(id.as_str(), start..index.entries.len());
        }
        index
    }

    fn push(
        &mut self,
        node: &'a Node,
        parent: Option<&'a NodeId>,
        page: Option<&'a Page>,
        component: Option<(&'a ComponentId, &'a Component)>,
    ) {
        let at = self.entries.len();
        self.by_id.entry(node.id().as_str()).or_insert(at);
        self.entries.push(Entry {
            node,
            parent,
            page,
            component,
            end: at + 1,
        });
        for child in node.children() {
            self.push(child, Some(node.id()), page, component);
        }
        self.entries[at].end = self.entries.len();
    }

    /// The position of the node `id`.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    /// The entry for the node `id`.
    pub fn get(&self, id: &str) -> Option<&Entry<'a>> {
        self.position(id).map(|at| &self.entries[at])
    }

    /// The entries a page, a component or a node covers (a node covers its subtree). `None` for an unknown id.
    pub fn scope(&self, id: &str) -> Option<Range<usize>> {
        if let Some(range) = self.pages.get(id).or_else(|| self.components.get(id)) {
            return Some(range.clone());
        }
        let at = self.position(id)?;
        Some(at..self.entries[at].end)
    }

    /// The entry at `at` and then its parents, nearest first.
    pub fn ancestors_or_self(&self, at: usize) -> Vec<usize> {
        let mut out = vec![at];
        let mut current = at;
        while let Some(parent) = self.entries[current].parent {
            match self.position(parent.as_str()) {
                Some(next) if !out.contains(&next) => {
                    out.push(next);
                    current = next;
                }
                _ => break,
            }
        }
        out
    }

    /// A short description of the node at `at`: enough to pick it out, nothing more.
    pub fn summary(&self, at: usize) -> Value {
        let entry = &self.entries[at];
        let mut out = json!({
            "id": entry.node.id().as_str(),
            "name": entry.node.name(),
            "type": entry.node.kind().as_str(),
        });
        let map = out.as_object_mut().expect("an object");
        if let Some(page) = entry.page {
            map.insert("page".into(), json!(page.id.as_str()));
        }
        if let Some((id, _)) = entry.component {
            map.insert("component".into(), json!(id.as_str()));
        }
        if let Some(parent) = entry.parent {
            map.insert("parent".into(), json!(parent.as_str()));
        }
        out
    }
}

/// A node's own text, whatever kind it is: the characters of a text node, the content of a note.
pub fn node_text(node: &Node) -> Option<String> {
    match node {
        Node::Text(text) => Some(text.content.iter().map(|run| run.text.as_str()).collect()),
        Node::Note(note) => Some(note.content.clone()),
        Node::Context(context) => Some(context.content.clone()),
        Node::Prompt(prompt) => Some(prompt.content.clone()),
        _ => None,
    }
}

/// `text` cut to `limit` characters, with a marker when it was cut.
pub fn clip(text: &str, limit: usize) -> (String, bool) {
    if text.chars().count() <= limit {
        return (text.to_owned(), false);
    }
    let cut: String = text.chars().take(limit).collect();
    (format!("{cut}… [cut at {limit} characters]"), true)
}
