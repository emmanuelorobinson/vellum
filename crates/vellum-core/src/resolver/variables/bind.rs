//! Bind-time checks: a property bound to a variable needs a variable that exists and has a type the
//! property takes. These need no theme (a variable has one type whatever its value is), so the op layer
//! runs them when a binding is made, and `check_document` runs them over a whole document.

use std::collections::BTreeSet;
use std::fmt;

use vellum_types::{
    BindingPath, Bindings, Color, Component, Document, Node, NodeId, Override, Scalar, Style,
    Uniform, Value, VariableId, VariableType, Variant,
};

use super::{Bindable, type_list};

/// What is wrong with one binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingProblemKind {
    /// The document has no such variable.
    Unknown,
    /// The variable has a type the property does not take.
    Type {
        /// The types the property takes.
        expected: &'static [VariableType],
        /// The variable's type.
        found: VariableType,
    },
}

/// A binding that cannot stand: where it is, the variable it names and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingProblem {
    /// The place inside the node (or style), in the file's spelling: `fills[0].color`.
    pub at: String,
    /// The variable the property refers to.
    pub variable: VariableId,
    /// What is wrong.
    pub kind: BindingProblemKind,
}

impl BindingProblem {
    /// The variable's type, when the problem is a type the property does not take.
    pub fn found(&self) -> Option<VariableType> {
        match &self.kind {
            BindingProblemKind::Type { found, .. } => Some(*found),
            BindingProblemKind::Unknown => None,
        }
    }
}

impl fmt::Display for BindingProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            BindingProblemKind::Unknown => write!(
                f,
                "{} refers to variable {}, which does not exist",
                self.at, self.variable
            ),
            BindingProblemKind::Type { expected, found } => write!(
                f,
                "{} refers to variable {}, a {} variable; it takes {}",
                self.at,
                self.variable,
                found.as_str(),
                type_list(expected)
            ),
        }
    }
}

struct Check<'a> {
    doc: &'a Document,
    problems: Vec<BindingProblem>,
}

impl Check<'_> {
    fn one<T: Bindable>(&mut self, at: &BindingPath<'_>, value: &Value<T>) {
        let Value::Var(reference) = value else {
            return;
        };
        let kind = match self.doc.variables.get(reference.id()) {
            None => BindingProblemKind::Unknown,
            Some(variable) if !T::ACCEPTS.contains(&variable.variable_type()) => {
                BindingProblemKind::Type {
                    expected: T::ACCEPTS,
                    found: variable.variable_type(),
                }
            }
            Some(_) => return,
        };
        self.problems.push(BindingProblem {
            at: at.to_string(),
            variable: reference.id().clone(),
            kind,
        });
    }
}

impl Bindings for Check<'_> {
    fn number(&mut self, at: &BindingPath<'_>, value: &Value<f64>) {
        self.one(at, value);
    }
    fn boolean(&mut self, at: &BindingPath<'_>, value: &Value<bool>) {
        self.one(at, value);
    }
    fn color(&mut self, at: &BindingPath<'_>, value: &Value<Color>) {
        self.one(at, value);
    }
    fn string(&mut self, at: &BindingPath<'_>, value: &Value<String>) {
        self.one(at, value);
    }
    fn scalar(&mut self, at: &BindingPath<'_>, value: &Value<Scalar>) {
        self.one(at, value);
    }
    fn uniform(&mut self, at: &BindingPath<'_>, value: &Value<Uniform>) {
        self.one(at, value);
    }
}

/// The bindings on `node` itself (not its children) that cannot stand in `doc`.
pub fn check_node_bindings(doc: &Document, node: &Node) -> Vec<BindingProblem> {
    let mut check = Check {
        doc,
        problems: Vec::new(),
    };
    node.visit_bindings(&mut check);
    check.problems
}

/// The bindings in `style` that cannot stand in `doc`.
pub fn check_style_bindings(doc: &Document, style: &Style) -> Vec<BindingProblem> {
    let mut check = Check {
        doc,
        problems: Vec::new(),
    };
    style.visit_bindings(&mut check);
    check.problems
}

/// Calls `f` on `node`, on what an instance puts in its slots and on the nodes its overrides replace others
/// with, and (when `children` is set) on everything below `node`. A props override is plain JSON and is not
/// walked.
fn walk<'a>(node: &'a Node, children: bool, f: &mut impl FnMut(&'a Node)) {
    f(node);
    if children {
        for child in node.children() {
            walk(child, true, f);
        }
    }
    if let Node::Instance(instance) = node {
        for content in instance.slot_content.values().flatten() {
            walk(content, true, f);
        }
        for over in instance.overrides.values() {
            walk_override(over, f);
        }
    }
}

fn walk_override<'a>(over: &'a Override, f: &mut impl FnMut(&'a Node)) {
    match over {
        Override::Props(_) => {}
        Override::Replace(node) => walk(node, true, f),
        Override::Children(nodes) => nodes.iter().for_each(|node| walk(node, true, f)),
    }
}

fn collect(doc: &Document, root: &Node, children: bool) -> Vec<(NodeId, BindingProblem)> {
    let mut found = Vec::new();
    walk(root, children, &mut |node| {
        found.extend(
            check_node_bindings(doc, node)
                .into_iter()
                .map(|problem| (node.id().clone(), problem)),
        );
    });
    found
}

/// The bindings anywhere in `root`'s subtree that cannot stand in `doc`, with the node each is on.
pub fn check_subtree_bindings(doc: &Document, root: &Node) -> Vec<(NodeId, BindingProblem)> {
    collect(doc, root, true)
}

/// As [`check_subtree_bindings`], but not below `root`'s children: `root` itself, and the content its slots
/// and overrides carry. This is what an update to one node can change.
pub fn check_own_bindings(doc: &Document, root: &Node) -> Vec<(NodeId, BindingProblem)> {
    collect(doc, root, false)
}

/// The bindings in a component's tree and in its variants' overrides that cannot stand in `doc`.
pub fn check_component_bindings(
    doc: &Document,
    component: &Component,
) -> Vec<(NodeId, BindingProblem)> {
    let mut found = check_subtree_bindings(doc, &component.root);
    found.extend(check_variants_bindings(doc, &component.variants));
    found
}

/// The bindings in the nodes `variants` carry in their overrides that cannot stand in `doc`.
pub fn check_variants_bindings(
    doc: &Document,
    variants: &[Variant],
) -> Vec<(NodeId, BindingProblem)> {
    let mut found = Vec::new();
    for over in variants.iter().flat_map(|v| v.overrides.values()) {
        walk_override(over, &mut |node| {
            found.extend(
                check_node_bindings(doc, node)
                    .into_iter()
                    .map(|problem| (node.id().clone(), problem)),
            );
        });
    }
    found
}

/// Every binding in `doc` that names a variable of a type its property does not take, as plain sentences
/// naming the layer, component or style. A reference to a variable that does not exist is not listed:
/// `check_document` reports those itself, and reports the ones in props overrides this walk cannot see.
pub fn check_document_bindings(doc: &Document) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut add = |place: String, found: Vec<(NodeId, BindingProblem)>| {
        for (node, problem) in found {
            if problem.found().is_some() {
                sentences.push(format!("{place}, layer {node}: {problem}"));
            }
        }
    };
    for page in &doc.pages {
        for root in &page.children {
            add(
                format!("on page {}", page.id),
                check_subtree_bindings(doc, root),
            );
        }
    }
    for (id, component) in &doc.components {
        add(
            format!("in component {id}"),
            check_component_bindings(doc, component),
        );
    }
    for (id, style) in &doc.styles {
        for problem in check_style_bindings(doc, style) {
            if problem.found().is_some() {
                sentences.push(format!("in style {id}: {problem}"));
            }
        }
    }
    sentences
}

/// The first binding in `doc` that refers to one of `variables` as a type its property does not take.
pub fn first_misuse_of(doc: &Document, variables: &BTreeSet<VariableId>) -> Option<BindingProblem> {
    let mut found = Vec::new();
    for page in &doc.pages {
        for root in &page.children {
            found.extend(check_subtree_bindings(doc, root));
        }
    }
    for component in doc.components.values() {
        found.extend(check_component_bindings(doc, component));
    }
    let styles = doc
        .styles
        .values()
        .flat_map(|s| check_style_bindings(doc, s));
    found
        .into_iter()
        .map(|(_, problem)| problem)
        .chain(styles)
        .find(|p| p.found().is_some() && variables.contains(&p.variable))
}
