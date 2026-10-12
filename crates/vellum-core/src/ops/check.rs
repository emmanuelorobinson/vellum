//! What "valid" means for a whole document, as far as the op layer keeps it so: ids are unique across pages
//! and components, every node may sit in its parent, every `$var` names a variable of this document, and
//! aliases name a variable of the same type without coming back. Invariant 1's property test checks every
//! accepted batch against this. Instances naming a component that exists is Prompt 12's and 17's.

use std::collections::BTreeSet;

use vellum_types::{Document, referenced_variables};

use super::edit::{Container, all_ids, check_kinds};
use super::lower::alias_cycle;

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
    problems
}
