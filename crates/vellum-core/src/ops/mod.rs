//! Ops: the only way to change a document (guide Invariant 1; ARCHITECTURE §5).
//!
//! A [`Transaction`] is an [`Origin`] and a list of [`Op`]s. [`plan`] checks the ops in order against a copy of
//! the document, each seeing the ones before it, and lowers them to [`Edit`]s with their inverse; any failure
//! refuses the whole batch and names the op and the path inside it. [`apply`] hands the result to a
//! [`DocStore`] as a [`Commit`], which nothing outside this module can build, so `DocStore::apply` is reachable
//! from here alone (the trybuild tests in `tests/write-path/` prove it).
//!
//! Structural, component, slot and geometric operations all use this transaction path.

pub(crate) mod advanced;
mod check;
pub(crate) mod edit;
mod lower;
#[cfg(test)]
pub(crate) mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vellum_types::{
    AssetId, BoolOp, Component, ComponentId, Document, DocumentMeta, FontFace, Node, NodeId,
    NodeIdPath, NodeKind, NodePatch, Origin, Page, PageId, PageKind, Point, PropDef, Style,
    StyleId, Variable, VariableId, VariableType, Variant,
};

pub use check::check_document;
pub use edit::{Container, Edit, Location, Tree, apply_edits, can_hold, find, locate};

use crate::ports::{ChangeSet, Commit, DocStore, IdGen, Revert};

/// What a document holds besides its pages, variables and components: the meta block, imports, themes, styles
/// and fonts. One op sets it whole; the merge works on its entries one by one.
#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    /// The meta block.
    pub meta: DocumentMeta,
    /// Library imports by alias.
    pub imports: BTreeMap<String, String>,
    /// Theme axes and their values.
    pub themes: BTreeMap<String, Vec<String>>,
    /// Text and effect styles.
    pub styles: BTreeMap<StyleId, Style>,
    /// Custom fonts.
    pub fonts: BTreeMap<AssetId, FontFace>,
}

impl Shell {
    /// The shell of `doc`.
    pub fn of(doc: &Document) -> Self {
        Self {
            meta: doc.meta.clone(),
            imports: doc.imports.clone(),
            themes: doc.themes.clone(),
            styles: doc.styles.clone(),
            fonts: doc.fonts.clone(),
        }
    }

    /// Puts this shell into `doc` and returns the one it had.
    pub(crate) fn swap_into(self, doc: &mut Document) -> Self {
        Self {
            meta: std::mem::replace(&mut doc.meta, self.meta),
            imports: std::mem::replace(&mut doc.imports, self.imports),
            themes: std::mem::replace(&mut doc.themes, self.themes),
            styles: std::mem::replace(&mut doc.styles, self.styles),
            fonts: std::mem::replace(&mut doc.fonts, self.fonts),
        }
    }
}

/// What holds a placed node: the top level of a page, or a container node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parent {
    /// The top level of a page.
    Page(PageId),
    /// A frame, group or boolean (in a page or a component).
    Node(NodeId),
}

/// Where a node goes: a parent and an index among its children, back to front. For a move, the index is where
/// the node ends up, counted after it has left its old place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The parent.
    pub parent: Parent,
    /// The index; equal to the number of children appends.
    pub index: usize,
}

/// An axis in the parent's coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
}
/// The edge or centre to align.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    /// Left or top.
    Start,
    /// Centre.
    Center,
    /// Right or bottom.
    End,
}

/// Derived geometry and resolved numbers supplied by the composition root. Bounds and paths are in
/// the node's parent coordinates, before rotation/flip; callers resolve layout, themes and text.
/// Missing derived data is refused, never guessed. Fixed literal boxes need no entry.
#[derive(Default)]
pub struct Inputs<'a> {
    /// Successful derived script output, never document content. ConvertScript snapshots it.
    pub script_outputs: BTreeMap<NodeId, Vec<Node>>,
    /// Measured boxes (needed for hug/fill and bound dimensions).
    pub bounds: BTreeMap<NodeId, crate::ports::Rect>,
    /// Theme-resolved numeric variables.
    pub numbers: BTreeMap<VariableId, f64>,
    /// Paths transformed into parent coordinates.
    pub paths: BTreeMap<NodeId, crate::ports::Path>,
    /// The path adapter used for booleans and outlines.
    pub path_ops: Option<&'a dyn crate::ports::PathOps>,
}

/// One change to a document.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Adds a node and its subtree. No id in it may exist yet.
    Insert {
        /// Where.
        to: Place,
        /// The node.
        node: Box<Node>,
    },
    /// Sets properties of a node; `null` puts one back to its default.
    Update {
        /// The node.
        id: NodeId,
        /// The properties.
        patch: NodePatch,
    },
    /// Removes a node and its subtree.
    Delete {
        /// The node.
        id: NodeId,
    },
    /// Moves a node, to another parent or page. It cannot leave its component, or enter one.
    Move {
        /// The node.
        id: NodeId,
        /// Where it ends up.
        to: Place,
    },
    /// Copies a node and its subtree with fresh ids from `IdGen`; [`Applied::created`] names the copy.
    Copy {
        /// The node to copy.
        id: NodeId,
        /// Where the copy goes.
        to: Place,
    },
    /// Removes a node and its subtree and puts `node` in its place. `node` may reuse the old ids.
    Replace {
        /// The node to replace.
        id: NodeId,
        /// What goes there instead.
        node: Box<Node>,
    },
    /// Adds or changes variables, and removes others. A variable still referred to cannot be removed.
    SetVariables {
        /// Variables to add or change.
        set: BTreeMap<VariableId, Variable>,
        /// Variables to remove.
        remove: BTreeSet<VariableId>,
    },
    /// Moves a node among its siblings.
    Reorder {
        /// The node.
        id: NodeId,
        /// Where it ends up.
        index: usize,
    },
    /// Puts sibling nodes into a new, empty frame placed where the frontmost of them was. Positions are rebased
    /// on the frame's position when it has one.
    WrapInFrame {
        /// The nodes, siblings in one parent.
        ids: Vec<NodeId>,
        /// The frame: empty, not rotated or flipped.
        frame: Box<Node>,
    },
    /// As [`Op::WrapInFrame`], with a group.
    Group {
        /// The nodes, siblings in one parent.
        ids: Vec<NodeId>,
        /// The group: empty, not rotated or flipped.
        group: Box<Node>,
    },
    /// Puts a group's children where the group was and removes it. The group's own opacity, blend mode and
    /// effects go with it.
    Ungroup {
        /// The group: not rotated or flipped.
        id: NodeId,
    },
    /// Makes a local component with a fresh definition tree and leaves an instance with the same id.
    CreateComponent {
        /// The source element.
        id: NodeId,
        /// The unused component id.
        component_id: ComponentId,
    },
    /// Makes a component, or detaches its only instance and removes its definition. Refuses a
    /// definition still referenced elsewhere.
    ToggleComponent {
        /// Element or instance.
        id: NodeId,
        /// New component id, or the instance's existing component id.
        component_id: ComponentId,
    },
    /// Expands one instance; nested instances and variable bindings remain linked.
    Detach {
        /// The instance.
        id: NodeId,
    },
    /// Replaces a descendant through an override, leaving the main component untouched.
    ReplaceInInstance {
        /// Instance.
        id: NodeId,
        /// Exact path from the component root.
        path: NodeIdPath,
        /// Replacement subtree.
        node: Box<Node>,
    },
    /// Marks an empty, non-root frame in a component as a slot. Suggestions are advisory.
    MakeSlot {
        /// Empty frame.
        id: NodeId,
        /// Suggested local components.
        suggested: Vec<ComponentId>,
    },
    /// Scales all lengths, with a normalized anchor in [0, 1]². Layout sizes become fixed.
    Scale {
        /// Subtree root.
        id: NodeId,
        /// Positive finite factor.
        factor: f64,
        /// Anchor in the root's untransformed box.
        anchor: Point,
    },
    /// Combines sibling shapes in an editable boolean node; geometry is evaluated by PathOps.
    Boolean {
        /// At least two siblings, in document stacking order.
        ids: Vec<NodeId>,
        /// Id for the new boolean.
        id: NodeId,
        /// Combination.
        op: BoolOp,
    },
    /// Replaces a stroked leaf with its stroke outline as a filled vector.
    OutlineStroke {
        /// Stroked leaf.
        id: NodeId,
    },
    /// Aligns two or more absolute siblings to their collective bounds.
    Align {
        /// Siblings.
        ids: Vec<NodeId>,
        /// Axis.
        axis: Axis,
        /// Edge or centre.
        alignment: Alignment,
    },
    /// Spaces three or more absolute siblings with equal edge gaps, keeping the extremes fixed.
    Distribute {
        /// Siblings.
        ids: Vec<NodeId>,
        /// Axis.
        axis: Axis,
    },
    /// Mirrors a node about its own centre along the chosen axis.
    Flip {
        /// Node.
        id: NodeId,
        /// Axis.
        axis: Axis,
    },
    /// Replaces a script with a frame holding its successful derived output, with fresh ids.
    ConvertScript {
        /// Script node.
        id: NodeId,
    },
    /// Adds a page, with its nodes, at `index` among the pages. No id in it may exist yet.
    InsertPage {
        /// Where among the pages; equal to their number appends.
        index: usize,
        /// The page.
        page: Box<Page>,
    },
    /// Removes a page and every node on it.
    DeletePage {
        /// The page.
        id: PageId,
    },
    /// Renames a page or changes its kind. Its nodes stay, and must be allowed on a page of that kind.
    UpdatePage {
        /// The page.
        id: PageId,
        /// Its name.
        name: String,
        /// Its kind.
        kind: PageKind,
    },
    /// Moves a page among the pages.
    MovePage {
        /// The page.
        id: PageId,
        /// Where it ends up, counted after it has left its old place.
        index: usize,
    },
    /// Sets the document's meta block, imports, themes, styles and fonts.
    SetShell {
        /// All of them, as they should read.
        shell: Box<Shell>,
    },
    /// Adds a component definition, replaces one with its whole node tree, or removes one no instance uses.
    SetComponent {
        /// The component.
        id: ComponentId,
        /// The definition, or `None` to remove it.
        component: Option<Box<Component>>,
    },
    /// Changes a component's name, props and variants; its node tree stays as it is.
    UpdateComponent {
        /// The component.
        id: ComponentId,
        /// Its name.
        name: String,
        /// Its props.
        props: BTreeMap<String, PropDef>,
        /// Its variants.
        variants: Vec<Variant>,
    },
}

/// Changes applied together, all or nothing, as one undo step. It cannot exist without an origin
/// (Invariant 2).
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// Who.
    pub origin: Origin,
    /// What, in order.
    pub ops: Vec<Op>,
}

impl Transaction {
    /// A transaction by `origin`.
    pub fn new(origin: Origin, ops: Vec<Op>) -> Self {
        Self { origin, ops }
    }
}

/// Why a transaction was refused. Nothing was applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpError {
    /// Which op, counting from 0. Zero for a fault in the transaction itself.
    pub op_index: usize,
    /// The field inside that op at fault, as `to.parent`, `node.children[1]` or `patch.opacity`; empty for the
    /// transaction itself.
    pub path: String,
    /// What is wrong.
    pub kind: OpErrorKind,
}

impl OpError {
    /// The fault as a path into the transaction: `ops[2].patch.opacity`.
    pub fn location(&self) -> String {
        match (&self.kind, self.path.as_str()) {
            (OpErrorKind::Empty, _) => "ops".into(),
            (_, "") => format!("ops[{}]", self.op_index),
            (_, path) => format!("ops[{}].{path}", self.op_index),
        }
    }
}

/// What is wrong with an op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpErrorKind {
    /// The transaction has no ops.
    Empty,
    /// No such page.
    UnknownPage(PageId),
    /// The page id is taken.
    DuplicatePage(PageId),
    /// No such node.
    UnknownNode(NodeId),
    /// No such variable.
    UnknownVariable(VariableId),
    /// The id is taken.
    DuplicateId(NodeId),
    /// The index is past the end of the siblings.
    IndexOutOfRange {
        /// The index given.
        index: usize,
        /// The largest index allowed.
        len: usize,
    },
    /// The parent cannot hold children.
    NotAContainer(NodeId),
    /// The parent cannot hold this kind of node.
    NotAllowedIn {
        /// The node.
        id: NodeId,
        /// Its kind.
        kind: NodeKind,
        /// The parent.
        parent: Container,
    },
    /// A node would move into itself or its own subtree.
    Cycle(NodeId),
    /// A node would move between a page and a component, or between components.
    CrossesBoundary(NodeId),
    /// A component's root is moved and removed with the component (Prompt 12).
    ComponentRoot(NodeId),
    /// Wrapping needs at least one node.
    NoTargets,
    /// The same node is named twice.
    Repeated(NodeId),
    /// The nodes to wrap do not share one parent.
    NotSiblings(NodeId),
    /// The node is the wrong kind for this op.
    WrongKind {
        /// What the op needs.
        expected: NodeKind,
        /// What it got.
        found: NodeKind,
    },
    /// A wrapper must start empty.
    WrapperNotEmpty,
    /// Positions cannot be rebased through a rotation or flip.
    Transformed(NodeId),
    /// The patch does not leave a valid node.
    InvalidPatch {
        /// The property at fault.
        property: String,
        /// Why.
        message: String,
    },
    /// The variable is still referred to.
    VariableInUse(VariableId),
    /// An alias names a variable of another type.
    AliasType {
        /// The variable aliased.
        alias: VariableId,
        /// The alias's own type.
        expected: VariableType,
        /// The aliased variable's type.
        found: VariableType,
    },
    /// Aliases lead back to this variable.
    AliasCycle(VariableId),
    /// A property is bound to a variable of a type it does not take.
    BindingType {
        /// The variable.
        variable: VariableId,
        /// The types the property takes.
        expected: &'static [VariableType],
        /// The variable's type.
        found: VariableType,
    },
    /// No local component with this id.
    UnknownComponent(ComponentId),
    /// Component id is taken.
    DuplicateComponent(ComponentId),
    /// Component is still referenced.
    ComponentInUse(ComponentId),
    /// Invalid component/slot/geometry input, with a concrete reason.
    InvalidInput(String),
    /// A measured box, resolved value or geometry adapter is required.
    MissingInput(String),
    /// A later prompt owns this operation.
    NotImplemented(&'static str),
}

fn container_name(container: Container) -> String {
    match container {
        Container::Page(kind) => format!("a {} page", kind.as_str()),
        Container::Node(kind) => format!("a {}", kind.as_str()),
    }
}

impl fmt::Display for OpErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownComponent(id) => write!(f, "there is no local component {id}."),
            Self::DuplicateComponent(id) => write!(f, "component {id} already exists."),
            Self::ComponentInUse(id) => write!(f, "component {id} is still used."),
            Self::InvalidInput(reason) => f.write_str(reason),
            Self::MissingInput(reason) => write!(f, "missing resolved input: {reason}"),
            Self::NotImplemented(op) => write!(f, "{op}: not implemented yet"),
            Self::Empty => f.write_str("the change has nothing in it."),
            Self::UnknownPage(id) => write!(f, "there is no page {id}."),
            Self::DuplicatePage(id) => write!(f, "the page id {id} is already used."),
            Self::UnknownNode(id) => write!(f, "there is no layer {id}."),
            Self::UnknownVariable(id) => write!(f, "there is no variable {id}."),
            Self::DuplicateId(id) => write!(f, "the id {id} is already used."),
            Self::IndexOutOfRange { index, len } => {
                write!(f, "position {index} is past the end (the last is {len}).")
            }
            Self::NotAContainer(id) => write!(f, "layer {id} cannot hold layers."),
            Self::NotAllowedIn { id, kind, parent } => write!(
                f,
                "layer {id} is a {} and cannot go in {}.",
                kind.as_str(),
                container_name(*parent)
            ),
            Self::Cycle(id) => write!(f, "layer {id} cannot go inside itself."),
            Self::CrossesBoundary(id) => {
                write!(f, "layer {id} cannot move between a page and a component.")
            }
            Self::ComponentRoot(id) => write!(f, "layer {id} is a component's main layer."),
            Self::NoTargets => f.write_str("there are no layers to wrap."),
            Self::Repeated(id) => write!(f, "layer {id} is named twice."),
            Self::NotSiblings(id) => write!(f, "layer {id} is not beside the other layers."),
            Self::WrongKind { expected, found } => write!(
                f,
                "this needs a {}, not a {}.",
                expected.as_str(),
                found.as_str()
            ),
            Self::WrapperNotEmpty => f.write_str("the new container must start empty."),
            Self::Transformed(id) => {
                write!(
                    f,
                    "layer {id} is rotated or flipped, so positions cannot be kept."
                )
            }
            Self::InvalidPatch { property, message } if property.is_empty() => {
                write!(f, "{message}")
            }
            Self::InvalidPatch { property, message } => write!(f, "{property}: {message}"),
            Self::VariableInUse(id) => write!(f, "variable {id} is still used."),
            Self::AliasType {
                alias,
                expected,
                found,
            } => write!(
                f,
                "variable {alias} is a {} variable, not a {} one.",
                found.as_str(),
                expected.as_str()
            ),
            Self::AliasCycle(id) => write!(f, "variable {id} ends up referring to itself."),
            Self::BindingType {
                variable,
                expected,
                found,
            } => write!(
                f,
                "{}",
                crate::resolver::variables::VariableError::TypeMismatch {
                    variable: variable.clone(),
                    expected,
                    found: *found,
                }
            ),
        }
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            OpErrorKind::Empty => f.write_str("The change has nothing in it."),
            kind => write!(f, "Change {} was refused: {kind}", self.op_index + 1),
        }
    }
}

impl std::error::Error for OpError {}

/// A checked transaction: the document it leads to, the edits that get there and the edits that come back.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The document after the transaction.
    pub document: Document,
    /// The edits, in order.
    pub edits: Vec<Edit>,
    /// The inverse, in the order to apply it: applied after `edits`, it restores the document exactly.
    pub inverse: Vec<Edit>,
    /// What changes, attributed to the transaction's origin.
    pub changes: ChangeSet,
    /// The id of each copy or boolean container made, by the index of the op that made it.
    pub created: BTreeMap<usize, NodeId>,
}

/// Checks `tx` against `doc` and lowers it to edits. Reads only; nothing is applied anywhere.
pub fn plan(doc: &Document, tx: &Transaction, ids: &mut dyn IdGen) -> Result<Plan, OpError> {
    plan_with_inputs(doc, tx, ids, &Inputs::default())
}

/// As [`plan`], with derived geometry and resolved variable values from the engine.
pub fn plan_with_inputs(
    doc: &Document,
    tx: &Transaction,
    ids: &mut dyn IdGen,
    inputs: &Inputs<'_>,
) -> Result<Plan, OpError> {
    if tx.ops.is_empty() {
        return Err(OpError {
            op_index: 0,
            path: String::new(),
            kind: OpErrorKind::Empty,
        });
    }
    // References (component instances, override paths, slots) are checked once, against the document the
    // whole transaction leaves: a transaction is all or nothing, so only where it ends has to be valid, and ops
    // may come in any order that gets there (the diff relies on it).
    let mut recorded = Recording {
        ids,
        made: Vec::new(),
    };
    let mut planner = lower::Planner::new(doc.clone(), false);
    run(&mut planner, tx, &mut recorded, inputs)?;
    // Variable types are checked at the same place, for the same reason (see `check::new_binding_problem`).
    let broken = advanced::validate_references(&planner.doc).err();
    let plan = planner.finish(tx.origin.clone());
    let binding = match broken {
        None => check::new_binding_problem(doc, &plan.document, &plan.changes)
            .or_else(|| advanced::validate_instance_bindings(&plan.document).err()),
        Some(_) => None,
    };
    if let Some(kind) = broken.or_else(|| binding.clone()) {
        // Name the op after which things were first broken: plan again, checking after every op, with the same
        // ids.
        let mut replay = Replay(recorded.made.into_iter());
        let mut strict = lower::Planner::new(doc.clone(), true);
        if let Some(binding) = binding {
            strict = strict.blaming(binding);
        }
        return Err(run(&mut strict, tx, &mut replay, inputs)
            .err()
            .unwrap_or(OpError {
                op_index: tx.ops.len() - 1,
                path: "id".into(),
                kind,
            }));
    }
    Ok(plan)
}

fn run(
    planner: &mut lower::Planner,
    tx: &Transaction,
    ids: &mut dyn IdGen,
    inputs: &Inputs<'_>,
) -> Result<(), OpError> {
    for (op_index, op) in tx.ops.iter().enumerate() {
        planner
            .lower(op_index, op, ids, inputs)
            .map_err(|(path, kind)| OpError {
                op_index,
                path,
                kind,
            })?;
        planner.expanded_binding_error().map_err(|kind| OpError {
            op_index,
            path: "id".into(),
            kind,
        })?;
        if planner.strict() {
            advanced::validate_references(&planner.doc).map_err(|kind| OpError {
                op_index,
                path: "id".into(),
                kind,
            })?;
        }
    }
    Ok(())
}

/// A transaction built one op at a time against a document, each op checked as it is added (as [`plan`]
/// checks it, references aside: [`plan`] checks those over the whole transaction). For code in this crate that
/// works out ops from the state they leave, such as the diff.
pub(crate) struct Draft {
    planner: lower::Planner,
    ops: Vec<Op>,
}

impl Draft {
    pub(crate) fn new(doc: &Document) -> Self {
        Self {
            planner: lower::Planner::new(doc.clone(), false),
            ops: Vec::new(),
        }
    }

    /// The document as the ops so far leave it.
    pub(crate) fn doc(&self) -> &Document {
        &self.planner.doc
    }

    /// Adds `op`, or refuses it and leaves the draft as it was.
    pub(crate) fn push(&mut self, op: Op) -> Result<(), OpError> {
        let op_index = self.ops.len();
        self.planner
            .lower(op_index, &op, &mut NoIds, &Inputs::default())
            .map_err(|(path, kind)| OpError {
                op_index,
                path,
                kind,
            })?;
        self.ops.push(op);
        Ok(())
    }

    pub(crate) fn into_ops(self) -> Vec<Op> {
        self.ops
    }
}

/// For drafts, whose ops never make ids.
struct NoIds;

impl IdGen for NoIds {
    fn node_id(&mut self) -> NodeId {
        unreachable!("a draft's ops carry their own ids")
    }
}

/// Hands out ids from another generator and remembers them.
struct Recording<'a> {
    ids: &'a mut dyn IdGen,
    made: Vec<NodeId>,
}

impl IdGen for Recording<'_> {
    fn node_id(&mut self) -> NodeId {
        let id = self.ids.node_id();
        self.made.push(id.clone());
        id
    }
}

/// Hands out the ids a [`Recording`] remembered, in the same order.
struct Replay(std::vec::IntoIter<NodeId>);

impl IdGen for Replay {
    fn node_id(&mut self) -> NodeId {
        self.0
            .next()
            .expect("planning the same transaction again asks for the same ids")
    }
}

/// What an applied transaction did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// What changed, from the store.
    pub changes: ChangeSet,
    /// The id of each copy or boolean container made, by the index of the op that made it.
    pub created: BTreeMap<usize, NodeId>,
}

/// Checks `tx` against the store's document and applies it as one commit, or refuses it whole and leaves the
/// store untouched. The one write path into a [`DocStore`].
pub fn apply<S: DocStore + ?Sized>(
    store: &mut S,
    ids: &mut dyn IdGen,
    tx: Transaction,
) -> Result<Applied, OpError> {
    apply_with_inputs(store, ids, tx, &Inputs::default())
}

/// The one write path with derived geometry, for scale, booleans and stroke outlines.
pub fn apply_with_inputs<S: DocStore + ?Sized>(
    store: &mut S,
    ids: &mut dyn IdGen,
    tx: Transaction,
    inputs: &Inputs<'_>,
) -> Result<Applied, OpError> {
    let plan = plan_with_inputs(&store.snapshot(), &tx, ids, inputs)?;
    Ok(commit(store, tx.origin, plan))
}

/// Applies a [`Revert`] from a store's `undo_turn` as a new transaction attributed to whoever asked for it, or
/// refuses it whole when the document has changed so that it no longer applies. A revert that keeps everything
/// has nothing in it and is refused as empty.
pub fn revert<S: DocStore + ?Sized>(store: &mut S, revert: Revert) -> Result<Applied, OpError> {
    let (origin, edits) = revert.into_parts();
    if edits.is_empty() {
        return Err(OpError {
            op_index: 0,
            path: String::new(),
            kind: OpErrorKind::Empty,
        });
    }
    let mut doc = store.snapshot();
    let (inverse, changes) =
        apply_edits(&mut doc, &edits, origin.clone()).map_err(|(op_index, kind)| OpError {
            op_index,
            path: String::new(),
            kind,
        })?;
    let changes = store.apply(Commit::new(origin, edits, inverse, changes));
    Ok(Applied {
        changes,
        created: BTreeMap::new(),
    })
}

/// "Undo this turn" on `store`, asked for by `by`: the turn is undone outright when nothing later depends on it,
/// otherwise reverted as a new transaction by `by` that keeps what others built on. What every target does.
pub fn undo_turn<S: DocStore + ?Sized>(
    store: &mut S,
    turn: &vellum_types::TurnId,
    by: &vellum_types::Actor,
) -> Result<crate::ports::TurnUndone, OpError> {
    use crate::ports::{TurnUndone, UndoTurn};
    Ok(match store.undo_turn(turn, by) {
        UndoTurn::Nothing => TurnUndone::Nothing,
        UndoTurn::Undone(changes) => TurnUndone::Undone(changes),
        UndoTurn::Revert(revert_tx) => {
            let kept = revert_tx.kept().to_vec();
            TurnUndone::Reverted {
                applied: revert(store, revert_tx)?,
                kept,
            }
        }
    })
}

/// Hands a checked plan to the store. Only callers that planned against the store's current document may use it:
/// `apply` here and the Guard's `apply_guarded`.
pub(crate) fn commit<S: DocStore + ?Sized>(store: &mut S, origin: Origin, plan: Plan) -> Applied {
    let commit = Commit::new(origin, plan.edits, plan.inverse, plan.changes);
    let changes = store.apply(commit);
    Applied {
        changes,
        created: plan.created,
    }
}
