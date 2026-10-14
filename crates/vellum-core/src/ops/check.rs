//! What "valid" means for a whole document, as far as the op layer keeps it so: ids are unique across pages
//! and components, every node may sit in its parent, every `$var` names a variable of this document, and
//! aliases name a variable of the same type without coming back, and every property bound to a variable takes
//! that variable's type. Invariant 1's property test checks every accepted batch against this. Local component
//! references, override paths and slot placement are checked too.

use std::collections::{BTreeSet, HashMap};

use vellum_types::{Document, Node, NodeId, VariableId, referenced_variables};

use super::OpErrorKind;
use super::edit::{Container, all_ids, check_kinds};
use super::lower::alias_cycle;
use crate::ports::ChangeSet;
use crate::resolver::variables::{
    BindingProblem, BindingProblemKind, check_own_bindings, check_style_bindings,
    check_variants_bindings, first_misuse_of,
};

/// Every problem in `doc`, as plain sentences naming the ids involved. Empty for a valid document.
pub fn check_document(doc: &Document) -> Vec<String> {
    let mut problems = Vec::new();

    let mut seen = BTreeSet::new();
    for id in all_ids(doc) {
        if !seen.insert(id.clone()) {
            problems.push(format!("the id {id} is used more than once"));
        }
    }

    for page in &doc.pages {
        for node in &page.children {
            if let Err(kind) = check_kinds(node, Container::Page(page.kind)) {
                problems.push(format!("on page {}: {kind}", page.id));
            }
        }
    }
    for (id, component) in &doc.components {
        for node in component.root.children() {
            if let Err(kind) = check_kinds(node, Container::Node(component.root.kind())) {
                problems.push(format!("in component {id}: {kind}"));
            }
        }
    }

    let mut used = referenced_variables(&doc.pages);
    used.extend(referenced_variables(&doc.components));
    used.extend(referenced_variables(&doc.styles));
    for id in used.iter().filter(|id| !doc.variables.contains_key(*id)) {
        problems.push(format!("variable {id} is used but does not exist"));
    }

    problems.extend(crate::resolver::variables::check_document_bindings(doc));

    for (id, variable) in &doc.variables {
        for alias in variable.aliases() {
            match doc.variables.get(alias) {
                None => problems.push(format!(
                    "variable {id} aliases {alias}, which does not exist"
                )),
                Some(target) if target.variable_type() != variable.variable_type() => {
                    problems.push(format!("variable {id} aliases {alias}, of another type"));
                }
                Some(_) => {}
            }
        }
        if alias_cycle(doc, id) {
            problems.push(format!(
                "variable {id} refers back to itself through aliases"
            ));
        }
    }
    if let Err(error) = super::advanced::validate_references(doc) {
        problems.push(error.to_string());
    }
    if let Err(error) = super::advanced::validate_instance_bindings(doc) {
        problems.push(error.to_string());
    }
    problems
}

/// The nodes of `doc` with an id in `wanted`, found in one walk of the pages and components.
fn nodes_by_id<'a>(doc: &'a Document, wanted: &BTreeSet<&NodeId>) -> HashMap<&'a NodeId, &'a Node> {
    fn walk<'a>(
        node: &'a Node,
        wanted: &BTreeSet<&NodeId>,
        found: &mut HashMap<&'a NodeId, &'a Node>,
    ) {
        if wanted.contains(node.id()) {
            found.insert(node.id(), node);
        }
        node.children()
            .iter()
            .for_each(|child| walk(child, wanted, found));
    }
    let mut found = HashMap::new();
    for node in doc.pages.iter().flat_map(|page| &page.children) {
        walk(node, wanted, &mut found);
    }
    for component in doc.components.values() {
        walk(&component.root, wanted, &mut found);
    }
    found
}

fn refusal(problem: &BindingProblem) -> OpErrorKind {
    match &problem.kind {
        BindingProblemKind::Unknown => OpErrorKind::UnknownVariable(problem.variable.clone()),
        BindingProblemKind::Type { expected, found } => OpErrorKind::BindingType {
            variable: problem.variable.clone(),
            expected,
            found: *found,
        },
    }
}

/// A binding a transaction left wrong that it did not find wrong: a variable bound to a property of another
/// type, in a node, style or component variant it inserted or changed, or in any place that uses a variable
/// whose type it changed. `before` and `after` are the document the transaction started from and the one it
/// leaves; a problem `before` already had in the same place is not the transaction's.
pub(super) fn new_binding_problem(
    before: &Document,
    after: &Document,
    changes: &ChangeSet,
) -> Option<OpErrorKind> {
    let touched: BTreeSet<&NodeId> = changes.inserted.iter().chain(&changes.updated).collect();
    if !touched.is_empty() {
        let (old, new) = (nodes_by_id(before, &touched), nodes_by_id(after, &touched));
        for id in &touched {
            let Some(node) = new.get(id) else { continue };
            let known = old
                .get(id)
                .map(|n| check_own_bindings(before, n))
                .unwrap_or_default();
            if let Some((_, problem)) = check_own_bindings(after, node)
                .iter()
                .find(|found| !known.contains(found))
            {
                return Some(refusal(problem));
            }
        }
    }
    for id in &changes.components {
        let (Some(old), Some(new)) = (before.components.get(id), after.components.get(id)) else {
            if let Some(new) = after.components.get(id)
                && let Some((_, problem)) = check_variants_bindings(after, &new.variants).first()
            {
                return Some(refusal(problem));
            }
            continue;
        };
        let known = check_variants_bindings(before, &old.variants);
        if let Some((_, problem)) = check_variants_bindings(after, &new.variants)
            .iter()
            .find(|found| !known.contains(found))
        {
            return Some(refusal(problem));
        }
    }
    if changes.shell {
        for (id, style) in &after.styles {
            if before.styles.get(id) == Some(style) {
                continue;
            }
            let known = before
                .styles
                .get(id)
                .map(|s| check_style_bindings(before, s))
                .unwrap_or_default();
            if let Some(problem) = check_style_bindings(after, style)
                .iter()
                .find(|found| !known.contains(found))
            {
                return Some(refusal(problem));
            }
        }
    }
    let retyped: BTreeSet<VariableId> = changes
        .variables
        .iter()
        .filter(|id| {
            matches!(
                (before.variables.get(*id), after.variables.get(*id)),
                (Some(old), Some(new)) if old.variable_type() != new.variable_type()
            )
        })
        .cloned()
        .collect();
    if !retyped.is_empty() {
        return first_misuse_of(after, &retyped).as_ref().map(refusal);
    }
    None
}
