//! Resolving a tree: every node with its theme and its variable references replaced by values, cached per
//! subtree.
//!
//! A [`ResolvedNode`] is one node's own properties (as a `Node` without children, every `$var` that
//! resolved replaced by its literal), the theme it was resolved under, whether it is shown, and its resolved
//! children, shared by `Arc` so a rebuilt parent points at the same children as before.
//!
//! The [`Resolver`] keeps one entry per node. An entry is good while three things hold: the node's own
//! content did not change, the theme it inherits did not change, and none of the variables it read did. The
//! first two are the **dirty set** (a [`ChangeSet`] names the nodes that changed, and the inherited theme is
//! compared on the way down); the third is an index from variable to the nodes that read it, so a changed
//! variable reaches exactly its readers, through alias chains too, because a node's reads include every
//! variable on the chain it followed. Children are never trusted blindly: a parent is rebuilt whenever
//! one of its children's resolved nodes is not the one it held, which is how an insert, a removal, a move or
//! a reorder reaches the parent even though a [`ChangeSet`] names only the nodes that changed. The walk
//! visits every node, but only dirty ones are recomputed.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use vellum_types::{
    BindingPath, BindingsMut, Color, Document, Node, NodeId, Page, Scalar, Uniform, Value,
    VariableId,
};

use super::{Bindable, Theme, ThemeError, VariableError, resolve_value_tracked};
use crate::ports::ChangeSet;

/// Why something in a node could not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    /// A theme choice names an axis or value the document lacks.
    Theme(ThemeError),
    /// A variable reference could not be resolved.
    Variable(VariableError),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Theme(error) => error.fmt(f),
            Self::Variable(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Something a node holds that could not be resolved, and where. The node keeps the value it had: a
/// reference stays a reference, a bad theme choice is ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// The place in the node, in the file's spelling: `fills[0].color`, or `theme.Mode`.
    pub at: String,
    /// What went wrong.
    pub error: ResolveError,
}

/// A node as the layout and the display list read it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedNode {
    /// The node's own properties, without its children. Every `$var` that resolved is now a literal.
    pub node: Node,
    /// The theme in force at this node: what it inherited, with its own `theme` applied.
    pub theme: Theme,
    /// The node's own `visible`. A hidden ancestor hides this node too, which is for the reader of the tree
    /// to apply. A flag that could not be resolved counts as shown, and is in `problems`.
    pub visible: bool,
    /// What could not be resolved in this node alone (not its children).
    pub problems: Vec<Problem>,
    /// The resolved children, back to front.
    pub children: Vec<Arc<ResolvedNode>>,
}

impl ResolvedNode {
    /// Every problem in this subtree with the node it is on, in tree order.
    pub fn all_problems(&self) -> Vec<(&NodeId, &Problem)> {
        let mut found: Vec<(&NodeId, &Problem)> = self
            .problems
            .iter()
            .map(|problem| (self.node.id(), problem))
            .collect();
        for child in &self.children {
            found.extend(child.all_problems());
        }
        found
    }
}

/// Replaces each reference on a node with its value under a theme.
struct Substitute<'a> {
    doc: &'a Document,
    theme: &'a Theme,
    read: &'a mut BTreeSet<VariableId>,
    problems: &'a mut Vec<Problem>,
}

impl Substitute<'_> {
    fn one<T: Bindable + Clone>(&mut self, at: &BindingPath<'_>, value: &mut Value<T>) {
        if !matches!(value, Value::Var(_)) {
            return;
        }
        match resolve_value_tracked(self.doc, value, self.theme, self.read) {
            Ok(literal) => *value = Value::Literal(literal),
            Err(error) => self.problems.push(Problem {
                at: at.to_string(),
                error: ResolveError::Variable(error),
            }),
        }
    }
}

impl BindingsMut for Substitute<'_> {
    fn number(&mut self, at: &BindingPath<'_>, value: &mut Value<f64>) {
        self.one(at, value);
    }
    fn boolean(&mut self, at: &BindingPath<'_>, value: &mut Value<bool>) {
        self.one(at, value);
    }
    fn color(&mut self, at: &BindingPath<'_>, value: &mut Value<Color>) {
        self.one(at, value);
    }
    fn string(&mut self, at: &BindingPath<'_>, value: &mut Value<String>) {
        self.one(at, value);
    }
    fn scalar(&mut self, at: &BindingPath<'_>, value: &mut Value<Scalar>) {
        self.one(at, value);
    }
    fn uniform(&mut self, at: &BindingPath<'_>, value: &mut Value<Uniform>) {
        self.one(at, value);
    }
}

/// What the resolver remembers about one node.
struct Entry {
    /// The theme the node inherited when it was resolved.
    theme_in: Theme,
    resolved: Arc<ResolvedNode>,
    /// The variables its own properties read, including every one on the alias chains it followed.
    read: BTreeSet<VariableId>,
}

/// Resolves trees and remembers the result per node. See the module documentation for what keeps an entry
/// good. The caller owns the contract: tell it what changed ([`Resolver::invalidate`]) between resolving one
/// version of a document and the next, or resolve with a new `Resolver`.
#[derive(Default)]
pub struct Resolver {
    /// The axes the entries were resolved against; a change of axes drops every entry.
    themes: Option<BTreeMap<String, Vec<String>>>,
    entries: HashMap<NodeId, Entry>,
    /// The nodes whose entry read each variable.
    readers: HashMap<VariableId, HashSet<NodeId>>,
    computed: u64,
}

impl Resolver {
    /// A resolver with nothing cached.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many nodes this resolver has computed from scratch since it was made (a node served from the
    /// cache does not count, and neither does a parent rebuilt around children that changed).
    pub fn computed(&self) -> u64 {
        self.computed
    }

    /// How many nodes are cached.
    pub fn cached(&self) -> usize {
        self.entries.len()
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.readers.clear();
    }

    /// Tells the resolver what changed in the document it is about to resolve next: nodes the change set
    /// names, and every node that read a variable it names, are recomputed. A change to the theme axes
    /// drops everything (every default may have changed). Several change sets can be passed one after the
    /// other before a resolve.
    pub fn invalidate(&mut self, doc: &Document, changes: &ChangeSet) {
        self.sync_themes(doc);
        for id in changes
            .inserted
            .iter()
            .chain(&changes.updated)
            .chain(&changes.removed)
        {
            self.evict(id);
        }
        for variable in &changes.variables {
            if let Some(readers) = self.readers.remove(variable) {
                for id in readers {
                    self.evict(&id);
                }
            }
        }
    }

    /// Resolves the top level of `page` under the document's default theme.
    pub fn resolve_page(&mut self, doc: &Document, page: &Page) -> Vec<Arc<ResolvedNode>> {
        self.resolve_nodes(doc, &page.children)
    }

    /// Resolves `nodes` side by side under the document's default theme.
    pub fn resolve_nodes(&mut self, doc: &Document, nodes: &[Node]) -> Vec<Arc<ResolvedNode>> {
        self.sync_themes(doc);
        let theme = Theme::defaults(doc);
        nodes
            .iter()
            .map(|node| self.resolve_in(doc, node, &theme))
            .collect()
    }

    /// Resolves `node` and its subtree as if it inherited `theme`.
    pub fn resolve_node(
        &mut self,
        doc: &Document,
        node: &Node,
        theme: &Theme,
    ) -> Arc<ResolvedNode> {
        self.sync_themes(doc);
        self.resolve_in(doc, node, theme)
    }

    fn sync_themes(&mut self, doc: &Document) {
        if self.themes.as_ref() != Some(&doc.themes) {
            self.clear();
            self.themes = Some(doc.themes.clone());
        }
    }

    fn evict(&mut self, id: &NodeId) {
        let Some(entry) = self.entries.remove(id) else {
            return;
        };
        for variable in &entry.read {
            if let Some(readers) = self.readers.get_mut(variable) {
                readers.remove(id);
                if readers.is_empty() {
                    self.readers.remove(variable);
                }
            }
        }
    }

    fn store(&mut self, id: &NodeId, entry: Entry) {
        for variable in &entry.read {
            self.readers
                .entry(variable.clone())
                .or_default()
                .insert(id.clone());
        }
        self.entries.insert(id.clone(), entry);
    }

    fn resolve_in(&mut self, doc: &Document, node: &Node, theme_in: &Theme) -> Arc<ResolvedNode> {
        let id = node.id();
        if self
            .entries
            .get(id)
            .is_some_and(|e| e.theme_in != *theme_in)
        {
            self.evict(id);
        }
        let cached = self.entries.get(id).map(|e| e.resolved.clone());
        let fresh = match &cached {
            Some(_) => None,
            None => Some(self.resolve_own(doc, node, theme_in)),
        };
        let theme = match (&cached, &fresh) {
            (Some(resolved), _) => resolved.theme.clone(),
            (None, Some((resolved, _))) => resolved.theme.clone(),
            (None, None) => unreachable!("either cached or computed"),
        };
        let children: Vec<Arc<ResolvedNode>> = node
            .children()
            .iter()
            .map(|child| self.resolve_in(doc, child, &theme))
            .collect();
        match (cached, fresh) {
            (Some(old), _) => {
                let same = old.children.len() == children.len()
                    && old
                        .children
                        .iter()
                        .zip(&children)
                        .all(|(a, b)| Arc::ptr_eq(a, b));
                if same {
                    return old;
                }
                let rebuilt = Arc::new(ResolvedNode {
                    children,
                    ..(*old).clone()
                });
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.resolved = rebuilt.clone();
                }
                rebuilt
            }
            (None, Some((mut resolved, read))) => {
                resolved.children = children;
                let resolved = Arc::new(resolved);
                self.store(
                    id,
                    Entry {
                        theme_in: theme_in.clone(),
                        resolved: resolved.clone(),
                        read,
                    },
                );
                resolved
            }
            (None, None) => unreachable!("either cached or computed"),
        }
    }

    /// The node's own part, without children, and what it read.
    fn resolve_own(
        &mut self,
        doc: &Document,
        node: &Node,
        theme_in: &Theme,
    ) -> (ResolvedNode, BTreeSet<VariableId>) {
        self.computed += 1;
        let mut problems = Vec::new();
        let theme = match node.theme() {
            Some(choices) if !choices.is_empty() => {
                let mut errors = Vec::new();
                let theme = theme_in.with_lenient(doc, choices, &mut errors);
                problems.extend(errors.into_iter().map(|error| Problem {
                    at: format!("theme.{}", error.axis()),
                    error: ResolveError::Theme(error),
                }));
                theme
            }
            _ => theme_in.clone(),
        };
        let mut own = node.without_children();
        let mut read = BTreeSet::new();
        own.visit_bindings_mut(&mut Substitute {
            doc,
            theme: &theme,
            read: &mut read,
            problems: &mut problems,
        });
        let visible = match own.visible() {
            Value::Literal(shown) => *shown,
            Value::Var(_) => true,
        };
        (
            ResolvedNode {
                node: own,
                theme,
                visible,
                problems,
                children: Vec::new(),
            },
            read,
        )
    }
}
