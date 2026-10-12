//! Lowering each op to edits, with the checks that belong to the op rather than to the tree.

use std::collections::{BTreeMap, BTreeSet};

use vellum_types::{
    Document, Node, NodeId, NodeKind, Origin, Point, Position, Value, Variable, VariableId,
    referenced_variables,
};

use super::edit::{Location, Touched, apply_one, find, locate};
use super::{Edit, Op, OpErrorKind, Parent, Place, Plan};
use crate::ports::IdGen;

type Refusal = (String, OpErrorKind);

fn at(path: impl Into<String>) -> impl FnOnce(OpErrorKind) -> Refusal {
    let path = path.into();
    move |kind| (path, kind)
}

/// The path inside `root` (written `prefix`) to the node `target`: `node`, `node.children[1].children[0]`.
fn node_path(prefix: &str, root: &Node, target: &NodeId) -> String {
    fn walk(node: &Node, target: &NodeId, path: &mut String) -> bool {
        if node.id() == target {
            return true;
        }
        for (i, child) in node.children().iter().enumerate() {
            let len = path.len();
            path.push_str(&format!(".children[{i}]"));
            if walk(child, target, path) {
                return true;
            }
            path.truncate(len);
        }
        false
    }
    let mut path = prefix.to_owned();
    walk(root, target, &mut path);
    path
}

/// The path for an error from inserting `node` (written `field`) at `to`.
fn insert_path(kind: &OpErrorKind, field: &str, node: &Node) -> String {
    match kind {
        OpErrorKind::UnknownPage(_)
        | OpErrorKind::UnknownNode(_)
        | OpErrorKind::NotAContainer(_) => "to.parent".into(),
        OpErrorKind::IndexOutOfRange { .. } => "to.index".into(),
        OpErrorKind::DuplicateId(id) | OpErrorKind::NotAllowedIn { id, .. } => {
            node_path(field, node, id)
        }
        _ => field.into(),
    }
}

/// The path for an error from moving `id` to a place.
fn move_path(kind: &OpErrorKind, id: &NodeId) -> &'static str {
    match kind {
        OpErrorKind::UnknownNode(missing) if missing == id => "id",
        OpErrorKind::ComponentRoot(_) | OpErrorKind::NotAllowedIn { .. } => "id",
        OpErrorKind::IndexOutOfRange { .. } => "to.index",
        _ => "to.parent",
    }
}

fn transformed(node: &Node) -> bool {
    *node.rotation() != Value::Literal(0.0) || node.flip_x() || node.flip_y()
}

fn absolute(node: &Node) -> Option<Point> {
    match node.position() {
        Position::Absolute(point) => Some(*point),
        Position::Auto => None,
    }
}

/// A copy of `node` with every id fresh.
fn renumbered(node: &Node, ids: &mut dyn IdGen) -> Node {
    let mut copy = node.clone();
    fn walk(node: &mut Node, ids: &mut dyn IdGen) {
        let fresh = ids.node_id();
        set_id(node, fresh);
        if let Some(children) = node.children_mut() {
            children.iter_mut().for_each(|child| walk(child, ids));
        }
    }
    walk(&mut copy, ids);
    copy
}

// `Node` has no id setter on purpose (an id never changes); a copy is the one place a new node is made from an
// old one.
fn set_id(node: &mut Node, id: NodeId) {
    macro_rules! each {
        ($($kind:ident),*) => {
            match node { $(Node::$kind(n) => n.id = id,)* }
        };
    }
    each!(
        Frame, Group, Rect, Ellipse, Polygon, Star, Line, Vector, Boolean, Text, Image, Icon,
        Instance, Script, Note, Context, Prompt, Embed
    );
}

pub(super) struct Planner {
    doc: Document,
    edits: Vec<Edit>,
    inverse: Vec<Edit>,
    touched: Touched,
    created: BTreeMap<usize, NodeId>,
}

impl Planner {
    pub(super) fn new(doc: Document) -> Self {
        Self {
            doc,
            edits: Vec::new(),
            inverse: Vec::new(),
            touched: Touched::default(),
            created: BTreeMap::new(),
        }
    }

    pub(super) fn finish(mut self, origin: Origin) -> Plan {
        self.inverse.reverse();
        Plan {
            document: self.doc,
            edits: self.edits,
            inverse: self.inverse,
            changes: self.touched.into_change_set(origin),
            created: self.created,
        }
    }

    fn edit(&mut self, edit: Edit) -> Result<(), OpErrorKind> {
        let inverse = apply_one(&mut self.doc, &edit, &mut self.touched)?;
        self.edits.push(edit);
        self.inverse.push(inverse);
        Ok(())
    }

    fn located(&self, id: &NodeId, path: &str) -> Result<Location, Refusal> {
        let location = locate(&self.doc, id)
            .ok_or_else(|| (path.to_owned(), OpErrorKind::UnknownNode(id.clone())))?;
        if location.is_component_root() {
            return Err((path.to_owned(), OpErrorKind::ComponentRoot(id.clone())));
        }
        Ok(location)
    }

    fn variables_exist(&self, refs: BTreeSet<VariableId>, path: &str) -> Result<(), Refusal> {
        match refs
            .into_iter()
            .find(|id| !self.doc.variables.contains_key(id))
        {
            Some(missing) => Err((path.to_owned(), OpErrorKind::UnknownVariable(missing))),
            None => Ok(()),
        }
    }

    fn insert(&mut self, to: Place, node: &Node, field: &str) -> Result<(), Refusal> {
        self.edit(Edit::Insert {
            to,
            node: Box::new(node.clone()),
        })
        .map_err(|kind| (insert_path(&kind, field, node), kind))?;
        self.variables_exist(referenced_variables(node), field)
    }

    pub(super) fn lower(
        &mut self,
        op_index: usize,
        op: &Op,
        ids: &mut dyn IdGen,
    ) -> Result<(), Refusal> {
        match op {
            Op::Insert { to, node } => self.insert(to.clone(), node, "node"),
            Op::Update { id, patch } => {
                let current = find(&self.doc, id)
                    .ok_or_else(|| ("id".to_owned(), OpErrorKind::UnknownNode(id.clone())))?;
                let next = current.patched(patch).map_err(|e| {
                    let path = if e.property.is_empty() {
                        "patch".to_owned()
                    } else {
                        format!("patch.{}", e.property)
                    };
                    (
                        path,
                        OpErrorKind::InvalidPatch {
                            property: e.property,
                            message: e.message,
                        },
                    )
                })?;
                self.variables_exist(referenced_variables(patch), "patch")?;
                self.edit(Edit::Set {
                    node: Box::new(next),
                })
                .map_err(at("id"))
            }
            Op::Delete { id } => self.edit(Edit::Remove { id: id.clone() }).map_err(at("id")),
            Op::Move { id, to } => self
                .edit(Edit::Move {
                    id: id.clone(),
                    to: to.clone(),
                })
                .map_err(|kind| (move_path(&kind, id).to_owned(), kind)),
            Op::Reorder { id, index } => {
                let location = self.located(id, "id")?;
                let to = Place {
                    index: *index,
                    ..location.place()
                };
                self.edit(Edit::Move { id: id.clone(), to })
                    .map_err(|kind| match kind {
                        OpErrorKind::IndexOutOfRange { .. } => ("index".to_owned(), kind),
                        kind => ("id".to_owned(), kind),
                    })
            }
            Op::Copy { id, to } => {
                let source = find(&self.doc, id)
                    .ok_or_else(|| ("id".to_owned(), OpErrorKind::UnknownNode(id.clone())))?;
                let copy = renumbered(source, ids);
                let root = copy.id().clone();
                self.edit(Edit::Insert {
                    to: to.clone(),
                    node: Box::new(copy),
                })
                .map_err(|kind| match kind {
                    OpErrorKind::DuplicateId(_) | OpErrorKind::NotAllowedIn { .. } => {
                        ("id".to_owned(), kind)
                    }
                    OpErrorKind::IndexOutOfRange { .. } => ("to.index".to_owned(), kind),
                    kind => ("to.parent".to_owned(), kind),
                })?;
                self.created.insert(op_index, root);
                Ok(())
            }
            Op::Replace { id, node } => {
                let location = self.located(id, "id")?;
                self.edit(Edit::Remove { id: id.clone() })
                    .map_err(at("id"))?;
                self.insert(location.place(), node, "node")
            }
            Op::SetVariables { set, remove } => self.set_variables(set, remove),
            Op::WrapInFrame { ids, frame } => self.wrap(ids, frame, NodeKind::Frame, "frame"),
            Op::Group { ids, group } => self.wrap(ids, group, NodeKind::Group, "group"),
            Op::Ungroup { id } => self.ungroup(id),
        }
    }

    fn set_variables(
        &mut self,
        set: &BTreeMap<VariableId, Variable>,
        remove: &BTreeSet<VariableId>,
    ) -> Result<(), Refusal> {
        for (id, variable) in set {
            self.edit(Edit::SetVariable {
                id: id.clone(),
                variable: Some(Box::new(variable.clone())),
            })
            .map_err(at(format!("set.{id}")))?;
        }
        for id in remove {
            self.edit(Edit::SetVariable {
                id: id.clone(),
                variable: None,
            })
            .map_err(at(format!("remove.{id}")))?;
        }
        if !remove.is_empty() {
            let mut used = referenced_variables(&self.doc.pages);
            used.extend(referenced_variables(&self.doc.components));
            used.extend(referenced_variables(&self.doc.styles));
            for variable in self.doc.variables.values() {
                used.extend(variable.aliases().into_iter().cloned());
            }
            if let Some(id) = remove.iter().find(|id| used.contains(*id)) {
                return Err((
                    format!("remove.{id}"),
                    OpErrorKind::VariableInUse(id.clone()),
                ));
            }
        }
        for (id, variable) in set {
            check_aliases(&self.doc, id, variable).map_err(at(format!("set.{id}")))?;
        }
        Ok(())
    }

    fn wrap(
        &mut self,
        targets: &[NodeId],
        wrapper: &Node,
        expected: NodeKind,
        field: &str,
    ) -> Result<(), Refusal> {
        if targets.is_empty() {
            return Err(("ids".into(), OpErrorKind::NoTargets));
        }
        let mut seen = BTreeSet::new();
        let mut placed: Vec<(usize, NodeId)> = Vec::new();
        let mut first: Option<Location> = None;
        for (k, id) in targets.iter().enumerate() {
            let path = format!("ids[{k}]");
            if !seen.insert(id) {
                return Err((path, OpErrorKind::Repeated(id.clone())));
            }
            let location = self.located(id, &path)?;
            match &first {
                Some(first)
                    if (&first.tree, &first.parent) != (&location.tree, &location.parent) =>
                {
                    return Err((path, OpErrorKind::NotSiblings(id.clone())));
                }
                Some(_) => {}
                None => first = Some(location.clone()),
            }
            placed.push((location.index, id.clone()));
        }
        if wrapper.kind() != expected {
            return Err((
                field.into(),
                OpErrorKind::WrongKind {
                    expected,
                    found: wrapper.kind(),
                },
            ));
        }
        if !wrapper.children().is_empty() {
            return Err((field.into(), OpErrorKind::WrapperNotEmpty));
        }
        if transformed(wrapper) {
            return Err((field.into(), OpErrorKind::Transformed(wrapper.id().clone())));
        }
        placed.sort();
        let front = placed.last().expect("at least one").0;
        let to = Place {
            index: front + 1,
            ..first.expect("at least one").place()
        };
        self.insert(to, wrapper, field)?;
        let inside = Parent::Node(wrapper.id().clone());
        for (j, (_, id)) in placed.iter().enumerate() {
            self.edit(Edit::Move {
                id: id.clone(),
                to: Place {
                    parent: inside.clone(),
                    index: j,
                },
            })
            .map_err(at("ids"))?;
        }
        if let Some(origin) = absolute(wrapper) {
            for (_, id) in &placed {
                self.rebase(id, -origin.x, -origin.y).map_err(at("ids"))?;
            }
        }
        Ok(())
    }

    fn ungroup(&mut self, id: &NodeId) -> Result<(), Refusal> {
        let group = find(&self.doc, id)
            .ok_or_else(|| ("id".to_owned(), OpErrorKind::UnknownNode(id.clone())))?
            .clone();
        if group.kind() != NodeKind::Group {
            return Err((
                "id".into(),
                OpErrorKind::WrongKind {
                    expected: NodeKind::Group,
                    found: group.kind(),
                },
            ));
        }
        if transformed(&group) {
            return Err(("id".into(), OpErrorKind::Transformed(id.clone())));
        }
        let location = self.located(id, "id")?;
        let offset = absolute(&group);
        for (k, child) in group.children().iter().enumerate() {
            self.edit(Edit::Move {
                id: child.id().clone(),
                to: Place {
                    index: location.index + 1 + k,
                    ..location.place()
                },
            })
            .map_err(at("id"))?;
            if let Some(offset) = offset {
                self.rebase(child.id(), offset.x, offset.y)
                    .map_err(at("id"))?;
            }
        }
        self.edit(Edit::Remove { id: id.clone() }).map_err(at("id"))
    }

    /// Shifts an absolutely placed node by (dx, dy); a node placed by layout stays where it is.
    fn rebase(&mut self, id: &NodeId, dx: f64, dy: f64) -> Result<(), OpErrorKind> {
        let node = find(&self.doc, id).expect("just moved");
        let Some(point) = absolute(node) else {
            return Ok(());
        };
        let mut next = node.clone();
        next.set_position(Position::Absolute(Point {
            x: point.x + dx,
            y: point.y + dy,
        }));
        self.edit(Edit::Set {
            node: Box::new(next),
        })
    }
}

/// Every alias of `variable` names a variable of the same type, and following aliases never comes back to
/// `id`.
fn check_aliases(doc: &Document, id: &VariableId, variable: &Variable) -> Result<(), OpErrorKind> {
    for alias in variable.aliases() {
        let target = doc
            .variables
            .get(alias)
            .ok_or_else(|| OpErrorKind::UnknownVariable(alias.clone()))?;
        if target.variable_type() != variable.variable_type() {
            return Err(OpErrorKind::AliasType {
                alias: alias.clone(),
                expected: variable.variable_type(),
                found: target.variable_type(),
            });
        }
    }
    if alias_cycle(doc, id) {
        return Err(OpErrorKind::AliasCycle(id.clone()));
    }
    Ok(())
}

/// Whether following aliases from `id` comes back to it.
pub(super) fn alias_cycle(doc: &Document, id: &VariableId) -> bool {
    let mut stack: Vec<&VariableId> = doc
        .variables
        .get(id)
        .map(Variable::aliases)
        .unwrap_or_default();
    let mut seen = BTreeSet::new();
    while let Some(next) = stack.pop() {
        if next == id {
            return true;
        }
        if seen.insert(next)
            && let Some(variable) = doc.variables.get(next)
        {
            stack.extend(variable.aliases());
        }
    }
    false
}
