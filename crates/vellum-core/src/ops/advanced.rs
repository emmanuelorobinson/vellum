//! Component and geometric operations. Derived layout and path data are explicit inputs, not vendor types.
use super::lower::{Planner, Refusal, at, renumbered, set_id};
use super::{Alignment, Axis, Edit, Inputs, Op, OpErrorKind, Tree, find, locate};
use crate::ports::{IdGen, Path, PathSegment, Rect, StrokeStyle};
use std::collections::{BTreeMap, BTreeSet};
use vellum_types::{
    Component, ComponentId, Document, Node, NodeId, NodeIdPath, NodeKind, Override, Point,
    Position, Size, Slot, StrokeWidth, Value, ViewBox,
};

fn invalid(reason: impl Into<String>) -> OpErrorKind {
    OpErrorKind::InvalidInput(reason.into())
}
fn missing(reason: impl Into<String>) -> OpErrorKind {
    OpErrorKind::MissingInput(reason.into())
}
fn patch_error(e: vellum_types::PatchApplyError) -> OpErrorKind {
    OpErrorKind::InvalidPatch {
        property: e.property,
        message: e.message,
    }
}

fn nodes_owned(node: &Node, visit: &mut impl FnMut(&Node)) {
    visit(node);
    for child in node.children() {
        nodes_owned(child, visit);
    }
    if let Node::Instance(n) = node {
        for value in n.overrides.values() {
            match value {
                Override::Replace(node) => nodes_owned(node, visit),
                Override::Children(nodes) => {
                    for node in nodes {
                        nodes_owned(node, visit);
                    }
                }
                Override::Props(_) => {}
            }
        }
        for nodes in n.slot_content.values() {
            for node in nodes {
                nodes_owned(node, visit);
            }
        }
    }
}
fn instances_in(node: &Node, out: &mut BTreeSet<ComponentId>) {
    nodes_owned(node, &mut |node| {
        if let Node::Instance(n) = node
            && n.library.is_none()
        {
            out.insert(n.component_id.clone());
        }
    });
}
/// Reference checks include owned override and slot payloads. Cyclic definitions cannot resolve.
pub(crate) fn validate_references(doc: &Document) -> Result<(), OpErrorKind> {
    let mut owned = BTreeSet::new();
    let mut error = None;
    let mut check = |node: &Node| {
        nodes_owned(node, &mut |node| {
            if !owned.insert(node.id().clone()) {
                error = Some(OpErrorKind::DuplicateId(node.id().clone()));
            }
            if let Node::Frame(n) = node
                && n.slot.is_some()
            {
                let valid = locate(doc, &n.id).is_some_and(|loc| {
                    matches!(loc.tree, Tree::Component(_)) && !loc.is_component_root()
                });
                if !valid || !n.children.is_empty() {
                    error = Some(invalid(
                        "slots must be empty nonroot frames inside components",
                    ));
                }
                if let Some(slot) = &n.slot {
                    for id in &slot.suggested {
                        if !doc.components.contains_key(id) {
                            error = Some(OpErrorKind::UnknownComponent(id.clone()));
                        }
                    }
                }
            }
            if let Node::Instance(n) = node {
                if let Some(alias) = &n.library {
                    match doc.imports.get(alias) {
                        Some(path)
                            if crate::resolver::libraries::import_path("document.vellum", path)
                                .is_ok() => {}
                        _ => {
                            error = Some(invalid(format!(
                                "unknown or invalid library import {alias}"
                            )))
                        }
                    }
                    return;
                }
                match effective(doc, n) {
                    Ok(root) => {
                        if let Err(e) =
                            super::edit::check_kinds(&root, super::Container::Node(NodeKind::Frame))
                        {
                            error = Some(e);
                        }
                    }
                    Err(e) => error = Some(e),
                }
            }
        });
    };
    for page in &doc.pages {
        for node in &page.children {
            check(node);
        }
    }
    for c in doc.components.values() {
        check(&c.root);
    }
    if let Some(error) = error {
        return Err(error);
    }
    let mut refs = BTreeSet::new();
    for page in &doc.pages {
        for node in &page.children {
            instances_in(node, &mut refs);
        }
    }
    let mut edges = BTreeMap::new();
    for (id, c) in &doc.components {
        let mut children = BTreeSet::new();
        instances_in(&c.root, &mut children);
        refs.extend(children.clone());
        edges.insert(id.clone(), children);
    }
    if let Some(id) = refs.iter().find(|id| !doc.components.contains_key(*id)) {
        return Err(OpErrorKind::UnknownComponent(id.clone()));
    }
    for start in edges.keys() {
        let mut stack: Vec<_> = edges[start].iter().collect();
        let mut seen = BTreeSet::new();
        while let Some(next) = stack.pop() {
            if next == start {
                return Err(invalid(format!("component {start} refers back to itself")));
            }
            if seen.insert(next)
                && let Some(children) = edges.get(next)
            {
                stack.extend(children);
            }
        }
    }
    // Check every prop/default and variant, including definitions with no placed instances.
    for (id, component) in &doc.components {
        let Node::Instance(mut instance) = component.root.as_instance(id.clone()) else {
            unreachable!()
        };
        let root = effective(doc, &instance)?;
        validate_expanded(doc, &root, 0, false)?;
        for variant in &component.variants {
            instance.variant = variant.properties.clone();
            let root = effective(doc, &instance)?;
            validate_expanded(doc, &root, 0, false)?;
        }
    }
    for page in &doc.pages {
        for node in &page.children {
            validate_expanded(doc, node, 0, false)?;
        }
    }
    Ok(())
}
/// Bindings introduced by applying props overrides, including unselected variants. Called separately
/// from reference validation so the existing bind-time checker can retain its precise property paths.
pub(crate) fn validate_instance_bindings(doc: &Document) -> Result<(), OpErrorKind> {
    for (id, component) in &doc.components {
        let Node::Instance(mut instance) = component.root.as_instance(id.clone()) else {
            unreachable!()
        };
        let root = effective(doc, &instance)?;
        validate_expanded(doc, &root, 0, true)?;
        for variant in &component.variants {
            instance.variant = variant.properties.clone();
            validate_expanded(doc, &effective(doc, &instance)?, 0, true)?;
        }
    }
    for page in &doc.pages {
        for node in &page.children {
            validate_expanded(doc, node, 0, true)?;
        }
    }
    Ok(())
}
fn validate_expanded(
    doc: &Document,
    node: &Node,
    depth: usize,
    bindings: bool,
) -> Result<(), OpErrorKind> {
    if depth > 128 {
        return Err(invalid("instance depth exceeds 128"));
    }
    if let Node::Instance(instance) = node {
        if instance.library.is_some() {
            return Ok(());
        }
        let root = effective(doc, instance)?;
        return validate_expanded(doc, &root, depth + 1, bindings);
    }
    if bindings
        && let Some(problem) = crate::resolver::variables::check_node_bindings(doc, node).first()
    {
        return Err(match &problem.kind {
            crate::resolver::variables::BindingProblemKind::Unknown => {
                OpErrorKind::UnknownVariable(problem.variable.clone())
            }
            crate::resolver::variables::BindingProblemKind::Type { expected, found } => {
                OpErrorKind::BindingType {
                    variable: problem.variable.clone(),
                    expected,
                    found: *found,
                }
            }
        });
    }
    for child in node.children() {
        validate_expanded(doc, child, depth + 1, bindings)?;
    }
    Ok(())
}
fn definition_path<'a>(root: &'a Node, path: &NodeIdPath) -> Result<&'a Node, OpErrorKind> {
    let mut parts = path.segments();
    if parts.next() != Some(root.id()) {
        return Err(invalid("override path must start at the component root"));
    }
    let mut node = root;
    for id in parts {
        node = node
            .children()
            .iter()
            .find(|n| n.id() == id)
            .ok_or_else(|| OpErrorKind::UnknownNode(id.clone()))?;
    }
    Ok(node)
}
/// Whether an instance's component, variant, overrides and slot content resolve in `doc`.
pub(crate) fn resolves(doc: &Document, instance: &vellum_types::InstanceNode) -> bool {
    if let Some(alias) = &instance.library {
        return doc.imports.get(alias).is_some_and(|path| {
            crate::resolver::libraries::import_path("document.vellum", path).is_ok()
        });
    }
    effective(doc, instance).is_ok_and(|root| {
        super::edit::check_kinds(&root, super::Container::Node(NodeKind::Frame)).is_ok()
    })
}

fn effective(doc: &Document, instance: &vellum_types::InstanceNode) -> Result<Node, OpErrorKind> {
    if instance.library.is_some() {
        return Err(missing(
            "resolved library definition for component operation",
        ));
    }
    let c = doc
        .components
        .get(&instance.component_id)
        .ok_or_else(|| OpErrorKind::UnknownComponent(instance.component_id.clone()))?;
    crate::resolver::components::instantiate(c, instance).map_err(|e| invalid(e.to_string()))
}

fn clear_slots(node: &mut Node) {
    if let Node::Frame(n) = node {
        n.slot = None;
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            clear_slots(child);
        }
    }
}
fn number(value: &Value<f64>, inputs: &Inputs<'_>) -> Result<f64, OpErrorKind> {
    let n = match value {
        Value::Literal(n) => *n,
        Value::Var(v) => *inputs
            .numbers
            .get(v.id())
            .ok_or_else(|| missing(format!("number variable {}", v.id())))?,
    };
    if !n.is_finite() {
        return Err(invalid("numeric input must be finite"));
    }
    Ok(n)
}
fn bounds(node: &Node, inputs: &Inputs<'_>) -> Result<Rect, OpErrorKind> {
    let measured = inputs.bounds.get(node.id());
    let p = match node.position() {
        Position::Auto => measured
            .map(|b| Point { x: b.x, y: b.y })
            .unwrap_or_default(),
        Position::Absolute(p) => *p,
    };
    let dimension = |size: &Size, measured: Option<f64>| match size {
        Size::Fixed(v) => number(v, inputs),
        _ => measured.ok_or_else(|| missing(format!("measured bounds for {}", node.id()))),
    };
    let b = Rect::new(
        p.x,
        p.y,
        dimension(node.width(), measured.map(|b| b.width))?,
        dimension(node.height(), measured.map(|b| b.height))?,
    );
    if [b.x, b.y, b.width, b.height, b.right(), b.bottom()]
        .iter()
        .any(|n| !n.is_finite())
        || b.width < 0.
        || b.height < 0.
    {
        return Err(invalid("bounds must be finite and nonnegative"));
    }
    Ok(b)
}
fn scaled(
    node: &Node,
    doc: &Document,
    inputs: &Inputs<'_>,
    factor: f64,
    stack: &mut BTreeSet<ComponentId>,
    ids: &mut dyn IdGen,
) -> Result<Node, OpErrorKind> {
    let b = bounds(node, inputs)?;
    let patch = node
        .scaled_patch(factor, b.width * factor, b.height * factor, &|id| {
            inputs.numbers.get(id).copied()
        })
        .map_err(|reason| {
            if reason.starts_with("number variable") {
                missing(reason)
            } else {
                invalid(reason)
            }
        })?;
    let mut next = node.patched(&patch).map_err(patch_error)?;
    if let Position::Absolute(p) = node.position() {
        let p = Point {
            x: p.x * factor,
            y: p.y * factor,
        };
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(invalid("scaled position is not finite"));
        }
        next.set_position(Position::Absolute(p));
    }
    if let Node::Instance(n) = &mut next {
        if !stack.insert(n.component_id.clone()) {
            return Err(invalid("recursive instance cannot be scaled"));
        }
        let Node::Instance(original) = node else {
            unreachable!()
        };
        let root = effective(doc, original)?;
        let result = scaled(&root, doc, inputs, factor, stack, ids)?;
        fn overrides(
            before: &Node,
            after: &Node,
            path: NodeIdPath,
            original: &BTreeMap<NodeIdPath, Override>,
            owned: &BTreeMap<NodeIdPath, Override>,
            out: &mut BTreeMap<NodeIdPath, Override>,
            ids: &mut dyn IdGen,
        ) {
            match original.get(&path) {
                Some(Override::Replace(original)) => {
                    let mut replacement = if owned.contains_key(&path) {
                        after.clone()
                    } else {
                        renumbered(after, ids)
                    };
                    if owned.contains_key(&path) {
                        set_id(&mut replacement, original.id().clone());
                    }
                    out.insert(path, Override::Replace(Box::new(replacement)));
                    return;
                }
                Some(Override::Children(_)) => {
                    // A single override cannot express both new children and scaled container properties.
                    // Materialize this container with fresh ids, retaining all nested instance links.
                    out.insert(path, Override::Replace(Box::new(renumbered(after, ids))));
                    return;
                }
                _ => {}
            }
            let mut patch = after.property_patch();
            if let Some(Override::Props(old)) = original.get(&path) {
                let mut merged = old.clone();
                merged.overlay(&patch);
                patch = merged;
            }
            out.insert(path.clone(), Override::Props(patch));
            if matches!(before,Node::Frame(frame) if frame.slot.is_some()) {
                return;
            }
            for (before, after) in before.children().iter().zip(after.children()) {
                overrides(
                    before,
                    after,
                    path.clone().join(before.id().clone()),
                    original,
                    owned,
                    out,
                    ids,
                );
            }
        }
        let mut original_overrides = doc.components[&original.component_id]
            .variants
            .iter()
            .find(|v| v.properties == original.variant)
            .map(|v| v.overrides.clone())
            .unwrap_or_default();
        original_overrides.extend(original.overrides.clone());
        let mut patches = BTreeMap::new();
        overrides(
            &root,
            &result,
            NodeIdPath::from(root.id().clone()),
            &original_overrides,
            &original.overrides,
            &mut patches,
            ids,
        );
        n.overrides = patches;
        // Slot content is owned by the instance. It travels in its dedicated map, not inherited props.
        for children in n.slot_content.values_mut() {
            for child in children {
                *child = scaled(child, doc, inputs, factor, stack, ids)?;
            }
        }
        stack.remove(&n.component_id);
    }
    if let Some(children) = next.children_mut() {
        for child in children {
            *child = scaled(child, doc, inputs, factor, stack, ids)?;
        }
    }
    Ok(next)
}

impl Planner {
    fn node(&self, id: &NodeId) -> Result<Node, Refusal> {
        find(&self.doc, id)
            .cloned()
            .ok_or_else(|| ("id".into(), OpErrorKind::UnknownNode(id.clone())))
    }
    fn replace(&mut self, id: &NodeId, node: Node) -> Result<(), Refusal> {
        let place = self.located(id, "id")?.place();
        self.edit(Edit::Remove { id: id.clone() })
            .map_err(at("id"))?;
        self.insert(place, &node, "node")
    }
    fn set_tree(&mut self, node: &Node) -> Result<(), Refusal> {
        self.edit(Edit::Set {
            node: Box::new(node.clone()),
        })
        .map_err(at("id"))?;
        for child in node.children() {
            self.set_tree(child)?;
        }
        Ok(())
    }
    fn create_component(
        &mut self,
        id: &NodeId,
        component_id: &ComponentId,
        ids: &mut dyn IdGen,
    ) -> Result<(), Refusal> {
        self.located(id, "id")?;
        if self.doc.components.contains_key(component_id) {
            return Err((
                "component_id".into(),
                OpErrorKind::DuplicateComponent(component_id.clone()),
            ));
        }
        let source = self.node(id)?;
        if source.kind() == NodeKind::Instance {
            return Err((
                "id".into(),
                invalid("detach an instance before making it a component"),
            ));
        }
        let mut root = renumbered(&source, ids);
        if root.kind() != NodeKind::Frame {
            let mut child = root;
            child.set_position(Position::Absolute(Point::default()));
            root = source.as_frame(vec![child]);
            set_id(&mut root, ids.node_id());
            // Appearance lives on the original child rather than being painted twice.
            root = root
                .patched(&source.container_patch())
                .map_err(|e| ("id".into(), patch_error(e)))?;
        }
        root.set_position(Position::Auto);
        let component = Component {
            name: source.name().into(),
            props: BTreeMap::new(),
            root,
            variants: vec![],
        };
        self.edit(Edit::SetComponent {
            id: component_id.clone(),
            component: Some(Box::new(component)),
        })
        .map_err(at("component_id"))?;
        let mut instance = source.as_instance(component_id.clone());
        if source.kind() != NodeKind::Frame {
            instance = instance
                .patched(&source.container_patch())
                .map_err(|e| ("id".into(), patch_error(e)))?;
        }
        self.replace(id, instance)?;
        self.references("component_id")
    }
    fn detach(&mut self, id: &NodeId, ids: &mut dyn IdGen) -> Result<ComponentId, Refusal> {
        let Node::Instance(instance) = self.node(id)? else {
            return Err(("id".into(), invalid("detach needs an instance")));
        };
        let mut root = effective(&self.doc, &instance).map_err(at("id"))?;
        clear_slots(&mut root);
        root = renumbered(&root, ids);
        set_id(&mut root, id.clone());
        self.replace(id, root)?;
        Ok(instance.component_id)
    }
    fn targets(
        &self,
        ids: &[NodeId],
        minimum: usize,
        absolute: bool,
    ) -> Result<Vec<Node>, Refusal> {
        if ids.len() < minimum {
            return Err((
                "ids".into(),
                invalid(format!("this operation needs at least {minimum} layers")),
            ));
        }
        let mut seen = BTreeSet::new();
        let mut first = None;
        let mut nodes = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let path = format!("ids[{i}]");
            if !seen.insert(id) {
                return Err((path, OpErrorKind::Repeated(id.clone())));
            }
            let loc = self.located(id, &path)?;
            if first
                .as_ref()
                .is_some_and(|parent| parent != &(loc.tree.clone(), loc.parent.clone()))
            {
                return Err((path, OpErrorKind::NotSiblings(id.clone())));
            }
            first = Some((loc.tree, loc.parent));
            let n = find(&self.doc, id).expect("located");
            if absolute && matches!(n.position(), Position::Auto) {
                return Err((
                    path,
                    invalid("alignment and distribution need absolute positions"),
                ));
            }
            if *n.rotation() != Value::Literal(0.) || n.flip_x() || n.flip_y() {
                return Err((path, OpErrorKind::Transformed(id.clone())));
            }
            nodes.push(n.clone());
        }
        Ok(nodes)
    }
    pub(super) fn advanced(
        &mut self,
        op_index: usize,
        op: &Op,
        ids: &mut dyn IdGen,
        inputs: &Inputs<'_>,
    ) -> Result<(), Refusal> {
        match op {
            Op::CreateComponent { id, component_id } => {
                self.create_component(id, component_id, ids)
            }
            Op::ToggleComponent { id, component_id } => {
                if let Node::Instance(n) = self.node(id)? {
                    if &n.component_id != component_id {
                        return Err((
                            "component_id".into(),
                            invalid("component id must match the instance"),
                        ));
                    }
                    self.detach(id, ids)?;
                    let mut refs = BTreeSet::new();
                    for page in &self.doc.pages {
                        for node in &page.children {
                            instances_in(node, &mut refs);
                        }
                    }
                    for (cid, c) in &self.doc.components {
                        if cid != component_id {
                            instances_in(&c.root, &mut refs);
                        }
                    }
                    if refs.contains(component_id) {
                        return Err((
                            "component_id".into(),
                            OpErrorKind::ComponentInUse(component_id.clone()),
                        ));
                    }
                    self.edit(Edit::SetComponent {
                        id: component_id.clone(),
                        component: None,
                    })
                    .map_err(at("component_id"))
                } else {
                    self.create_component(id, component_id, ids)
                }
            }
            Op::Detach { id } => self.detach(id, ids).map(|_| ()),
            Op::ReplaceInInstance { id, path, node } => {
                let Node::Instance(mut n) = self.node(id)? else {
                    return Err(("id".into(), invalid("replacement needs an instance")));
                };
                let root = effective(&self.doc, &n).map_err(at("id"))?;
                if path.len() == 1 {
                    return Err((
                        "path".into(),
                        invalid("replace inside an instance needs a descendant"),
                    ));
                }
                definition_path(&root, path).map_err(at("path"))?;
                self.variables_exist(vellum_types::referenced_variables(node), "node")?;
                self.bindings_fit(node, "node")?;
                n.overrides
                    .insert(path.clone(), Override::Replace(node.clone()));
                // The replacement is validated in context (parent kind, ids and references).
                let candidate = effective(&self.doc, &n).map_err(at("node"))?;
                super::edit::check_kinds(&candidate, super::Container::Node(NodeKind::Frame))
                    .map_err(at("node"))?;
                self.edit(Edit::Set {
                    node: Box::new(Node::Instance(n)),
                })
                .map_err(at("id"))?;
                self.references("node")
            }
            Op::MakeSlot { id, suggested } => {
                let loc = locate(&self.doc, id)
                    .ok_or_else(|| ("id".into(), OpErrorKind::UnknownNode(id.clone())))?;
                if !matches!(loc.tree, Tree::Component(_)) || loc.is_component_root() {
                    return Err(("id".into(), invalid("a slot must be inside a component")));
                }
                let Node::Frame(mut n) = self.node(id)? else {
                    return Err(("id".into(), invalid("a slot must be a frame")));
                };
                if !n.children.is_empty() {
                    return Err(("id".into(), OpErrorKind::WrapperNotEmpty));
                }
                for (i, cid) in suggested.iter().enumerate() {
                    if !self.doc.components.contains_key(cid) {
                        return Err((
                            format!("suggested[{i}]"),
                            OpErrorKind::UnknownComponent(cid.clone()),
                        ));
                    }
                }
                n.slot = Some(Slot {
                    suggested: suggested.clone(),
                });
                self.edit(Edit::Set {
                    node: Box::new(Node::Frame(n)),
                })
                .map_err(at("id"))
            }
            Op::Scale { id, factor, anchor } => {
                if !factor.is_finite() || *factor <= 0. {
                    return Err((
                        "factor".into(),
                        invalid("scale factor must be positive and finite"),
                    ));
                }
                if [anchor.x, anchor.y]
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                {
                    return Err(("anchor".into(), invalid("anchor must be in [0, 1]")));
                }
                let node = self.node(id)?;
                let b = bounds(&node, inputs).map_err(at("id"))?;
                let mut next = scaled(&node, &self.doc, inputs, *factor, &mut BTreeSet::new(), ids)
                    .map_err(at("factor"))?;
                let cx = (1. - factor) * b.width / 2.;
                let cy = (1. - factor) * b.height / 2.;
                let dx = (1. - factor) * b.width * (anchor.x - 0.5);
                let dy = (1. - factor) * b.height * (anchor.y - 0.5);
                // Rotation and mirrors are about the box centre. Keep the chosen transformed anchor fixed.
                let angle = number(node.rotation(), inputs)
                    .map_err(at("id"))?
                    .to_radians();
                let dx = if node.flip_x() { -dx } else { dx };
                let dy = if node.flip_y() { -dy } else { dy };
                let point = Point {
                    x: b.x + cx + dx * angle.cos() - dy * angle.sin(),
                    y: b.y + cy + dx * angle.sin() + dy * angle.cos(),
                };
                if !point.x.is_finite() || !point.y.is_finite() {
                    return Err(("factor".into(), invalid("scaled position is not finite")));
                }
                if !matches!(node.position(), Position::Auto) || anchor.x != 0. || anchor.y != 0. {
                    next.set_position(Position::Absolute(point));
                } else {
                    next.set_position(Position::Auto);
                }
                self.set_tree(&next)
            }
            Op::Flip { id, axis } => {
                let node = self.node(id)?;
                let next = node
                    .patched(&node.flip_patch(*axis == Axis::Horizontal))
                    .map_err(|e| ("id".into(), patch_error(e)))?;
                self.edit(Edit::Set {
                    node: Box::new(next),
                })
                .map_err(at("id"))
            }
            Op::Align {
                ids,
                axis,
                alignment,
            } => self.align(ids, *axis, Some(*alignment), inputs),
            Op::Distribute { ids, axis } => self.align(ids, *axis, None, inputs),
            Op::Boolean {
                ids: targets,
                id,
                op,
            } => {
                let mut nodes = self.targets(targets, 2, false)?;
                nodes.sort_by_key(|n| locate(&self.doc, n.id()).expect("located").index);
                let adapter = inputs
                    .path_ops
                    .ok_or_else(|| ("op".into(), missing("PathOps")))?;
                let path = |n: &Node| {
                    inputs
                        .paths
                        .get(n.id())
                        .cloned()
                        .ok_or_else(|| ("ids".into(), missing(format!("path for {}", n.id()))))
                };
                let result = if *op == vellum_types::BoolOp::Subtract {
                    let mut rest = path(&nodes[1])?;
                    for n in &nodes[2..] {
                        rest = adapter.boolean(&rest, &path(n)?, vellum_types::BoolOp::Union);
                    }
                    adapter.boolean(&rest, &path(&nodes[0])?, *op)
                } else {
                    let mut result = path(&nodes[0])?;
                    for n in &nodes[1..] {
                        result = adapter.boolean(&result, &path(n)?, *op);
                    }
                    result
                };
                let b = path_bounds(adapter, &result).map_err(at("op"))?;
                checked_path(&result).map_err(at("op"))?;
                checked_rect(b).map_err(at("op"))?;
                let mut wrapper = nodes[0].as_boolean(*op);
                set_id(&mut wrapper, id.clone());
                wrapper.set_position(Position::Absolute(Point { x: b.x, y: b.y }));
                wrapper = wrapper
                    .patched(&wrapper.box_patch(b.width, b.height))
                    .map_err(|e| ("op".into(), patch_error(e)))?;
                self.wrap(targets, &wrapper, NodeKind::Boolean, "id")?;
                self.created.insert(op_index, id.clone());
                Ok(())
            }
            Op::OutlineStroke { id } => {
                let node = self.node(id)?;
                if !node.children().is_empty()
                    || matches!(node, Node::Instance(_) | Node::Script(_))
                {
                    return Err(("id".into(), invalid("outline stroke needs a leaf shape")));
                }
                let stroke = node
                    .stroke()
                    .ok_or_else(|| ("id".into(), invalid("layer has no stroke")))?;
                if !stroke.dash.is_empty() {
                    return Err((
                        "id".into(),
                        invalid("PathOps stroke style does not support dash patterns"),
                    ));
                }
                let StrokeWidth::Uniform(w) = &stroke.width else {
                    return Err((
                        "id".into(),
                        invalid("PathOps stroke style does not support per-side widths"),
                    ));
                };
                let width = number(w, inputs).map_err(at("id"))?;
                if width < 0. {
                    return Err(("id".into(), invalid("stroke width must be nonnegative")));
                }
                let style = StrokeStyle {
                    width,
                    align: stroke.align,
                    cap: stroke.cap,
                    join: stroke.join,
                };
                let path = inputs
                    .paths
                    .get(id)
                    .ok_or_else(|| ("id".into(), missing("resolved path")))?;
                let adapter = inputs
                    .path_ops
                    .ok_or_else(|| ("id".into(), missing("PathOps")))?;
                let outlined = adapter
                    .outline_stroke(path, &style)
                    .map_err(|e| ("id".into(), invalid(e)))?;
                checked_path(&outlined).map_err(at("id"))?;
                let b = path_bounds(adapter, &outlined).map_err(at("id"))?;
                checked_rect(b).map_err(at("id"))?;
                let mut next = node.as_vector(
                    svg(&outlined),
                    ViewBox {
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height,
                    },
                );
                next.set_position(Position::Absolute(Point { x: b.x, y: b.y }));
                next = next
                    .patched(&next.outline_patch(b.width, b.height, stroke.paints.clone()))
                    .map_err(|e| ("id".into(), patch_error(e)))?;
                self.replace(id, next)
            }
            Op::ConvertScript { id } => {
                let node = self.node(id)?;
                let Node::Script(script) = &node else {
                    return Err((
                        "id".into(),
                        OpErrorKind::WrongKind {
                            expected: NodeKind::Script,
                            found: node.kind(),
                        },
                    ));
                };
                let output = inputs
                    .script_outputs
                    .get(id)
                    .ok_or_else(|| ("id".into(), missing("successful script output")))?;
                if output.iter().map(crate::doc_api::count).sum::<usize>() > 1000 {
                    return Err(("id".into(), invalid("script output exceeds 1,000 nodes")));
                }
                let children = output.iter().map(|n| renumbered(n, ids)).collect();
                let mut frame = node.as_frame(children);
                if let Node::Frame(n) = &mut frame {
                    n.clip = script.clip;
                }
                self.replace(id, frame)
            }
            _ => unreachable!("structural ops are lowered in lower.rs"),
        }
    }
    fn align(
        &mut self,
        ids: &[NodeId],
        axis: Axis,
        alignment: Option<Alignment>,
        inputs: &Inputs<'_>,
    ) -> Result<(), Refusal> {
        let nodes = self.targets(ids, if alignment.is_some() { 2 } else { 3 }, true)?;
        let mut placed: Vec<_> = nodes
            .into_iter()
            .map(|n| bounds(&n, inputs).map(|b| (n, b)))
            .collect::<Result<_, _>>()
            .map_err(at("ids"))?;
        let start = |b: Rect| if axis == Axis::Horizontal { b.x } else { b.y };
        let size = |b: Rect| {
            if axis == Axis::Horizontal {
                b.width
            } else {
                b.height
            }
        };
        if let Some(alignment) = alignment {
            let union = placed
                .iter()
                .map(|(_, b)| *b)
                .reduce(|a, b| a.union(&b))
                .expect("targets");
            for (mut node, b) in placed {
                let offset = match alignment {
                    Alignment::Start => 0.,
                    Alignment::Center => (size(union) - size(b)) / 2.,
                    Alignment::End => size(union) - size(b),
                };
                let mut point = Point { x: b.x, y: b.y };
                if axis == Axis::Horizontal {
                    point.x = start(union) + offset;
                } else {
                    point.y = start(union) + offset;
                }
                node.set_position(Position::Absolute(point));
                self.edit(Edit::Set {
                    node: Box::new(node),
                })
                .map_err(at("ids"))?;
            }
        } else {
            placed.sort_by(|(a, x), (b, y)| {
                start(*x)
                    .total_cmp(&start(*y))
                    .then_with(|| a.id().cmp(b.id()))
            });
            let end =
                start(placed.last().expect("targets").1) + size(placed.last().expect("targets").1);
            let sum: f64 = placed.iter().map(|(_, b)| size(*b)).sum();
            let gap = (end - start(placed[0].1) - sum) / (placed.len() - 1) as f64;
            if !gap.is_finite() {
                return Err(("ids".into(), invalid("distributed gap is not finite")));
            }
            let mut cursor = start(placed[0].1);
            for (mut node, b) in placed {
                let mut point = Point { x: b.x, y: b.y };
                if axis == Axis::Horizontal {
                    point.x = cursor;
                } else {
                    point.y = cursor;
                }
                node.set_position(Position::Absolute(point));
                self.edit(Edit::Set {
                    node: Box::new(node),
                })
                .map_err(at("ids"))?;
                cursor += size(b) + gap;
            }
        }
        Ok(())
    }
}
fn checked_rect(b: Rect) -> Result<(), OpErrorKind> {
    if [b.x, b.y, b.width, b.height, b.right(), b.bottom()]
        .iter()
        .any(|n| !n.is_finite())
        || b.width < 0.
        || b.height < 0.
    {
        Err(invalid("PathOps returned invalid bounds"))
    } else {
        Ok(())
    }
}
fn checked_path(path: &Path) -> Result<(), OpErrorKind> {
    for segment in &path.segments {
        let points: &[Point] = match segment {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => std::slice::from_ref(p),
            PathSegment::QuadTo(a, b) => &[*a, *b],
            PathSegment::CubicTo(a, b, c) => &[*a, *b, *c],
            PathSegment::Close => &[],
        };
        if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(invalid("PathOps returned a nonfinite path"));
        }
    }
    Ok(())
}
fn svg(path: &Path) -> String {
    path.segments
        .iter()
        .map(|s| match s {
            PathSegment::MoveTo(p) => format!("M {} {}", p.x, p.y),
            PathSegment::LineTo(p) => format!("L {} {}", p.x, p.y),
            PathSegment::QuadTo(a, b) => format!("Q {} {} {} {}", a.x, a.y, b.x, b.y),
            PathSegment::CubicTo(a, b, c) => {
                format!("C {} {} {} {} {} {}", a.x, a.y, b.x, b.y, c.x, c.y)
            }
            PathSegment::Close => "Z".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn path_bounds(adapter: &dyn crate::ports::PathOps, path: &Path) -> Result<Rect, OpErrorKind> {
    match adapter.bounds(path, None) {
        Some(bounds) => {
            checked_rect(bounds)?;
            Ok(bounds)
        }
        None if path.is_empty() => Ok(Rect::default()),
        None => Err(invalid("PathOps returned no bounds for a nonempty path")),
    }
}
