//! The node union (ARCHITECTURE §4.1).
//!
//! Every kind of node is its own struct with the common fields first. Serde cannot combine
//! `flatten` with `deny_unknown_fields`, and unknown fields must be an error, so the common fields are
//! stamped onto each struct by [`node_struct!`] instead of living in a shared struct.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use ts_rs::TS;

use crate::color::Color;
use crate::component::Override;
use crate::defaults::{
    full_sweep, is_default, is_false, is_full_sweep, is_opaque, is_visible, opaque, visible,
};
use crate::effect::Effect;
use crate::enums::{
    Align, BlendMode, BoolOp, FillRule, ImageFit, Justify, Layout, NodeKind, TextAlign, TextResize,
    TransitionKind, VerticalAlign,
};
use crate::geometry::{CornerRadius, Padding, Point, Position, Size, Transform, ViewBox};
use crate::ids::{AssetId, ComponentId, NodeId, NodeIdPath, StyleId};
use crate::paint::Paint;
use crate::stroke::Stroke;
use crate::text::TextRun;
use crate::value::{Scalar, Value};

/// Every property a node can have, in the file's spelling. The `type` tag is not one of them: it names the
/// kind. A props override (`NodePatch`) may set any of these except the id and the children. A test checks
/// this list against the node schemas, so the two cannot drift apart.
pub const NODE_PROPERTIES: &[&str] = &[
    "id",
    "name",
    "visible",
    "locked",
    "opacity",
    "rotation",
    "flipX",
    "flipY",
    "position",
    "width",
    "height",
    "fills",
    "stroke",
    "effects",
    "blendMode",
    "agentNotes",
    "prototype",
    "metadata",
    "layout",
    "columns",
    "rows",
    "gap",
    "padding",
    "justify",
    "align",
    "layoutIncludesStroke",
    "clip",
    "cornerRadius",
    "theme",
    "slot",
    "children",
    "innerRadius",
    "startAngle",
    "sweepAngle",
    "sides",
    "points",
    "innerRatio",
    "path",
    "viewBox",
    "fillRule",
    "op",
    "content",
    "verticalAlign",
    "resize",
    "textStyle",
    "asset",
    "fit",
    "crop",
    "library",
    "icon",
    "weight",
    "fill",
    "componentId",
    "variant",
    "props",
    "overrides",
    "slotContent",
    "src",
    "inputs",
    "color",
    "pointsAt",
    "engine",
    "model",
    "url",
    "device",
    "zoom",
    "scroll",
    "radius",
];

/// Declares a node struct: the fields common to every node, then the ones for this kind.
macro_rules! node_struct {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $(#[$fmeta:meta])* $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        #[ts(export)]
        pub struct $name {
            /// Unique within the document, never reused.
            pub id: NodeId,
            /// The name shown in the layers panel.
            pub name: String,
            /// Shown or hidden; a boolean variable can decide.
            #[serde(default = "visible", skip_serializing_if = "is_visible")]
            pub visible: Value<bool>,
            /// Locked layers cannot be selected or moved on the canvas.
            #[serde(default, skip_serializing_if = "is_false")]
            pub locked: bool,
            /// 0 to 1.
            #[serde(default = "opaque", skip_serializing_if = "is_opaque")]
            pub opacity: Value<f64>,
            /// Degrees, clockwise.
            #[serde(default, skip_serializing_if = "is_default")]
            pub rotation: Value<f64>,
            /// Mirrored left to right.
            #[serde(default, skip_serializing_if = "is_false")]
            pub flip_x: bool,
            /// Mirrored top to bottom.
            #[serde(default, skip_serializing_if = "is_false")]
            pub flip_y: bool,
            /// Placed by the parent's layout, or at fixed coordinates.
            #[serde(default, skip_serializing_if = "is_default")]
            pub position: Position,
            /// Width: fixed, hug or fill.
            pub width: Size,
            /// Height: fixed, hug or fill.
            pub height: Size,
            /// Painted in order, first at the bottom.
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub fills: Vec<Paint>,
            /// The outline.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub stroke: Option<Stroke>,
            /// Applied in order.
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub effects: Vec<Effect>,
            /// How the layer combines with what is beneath it.
            #[serde(default, skip_serializing_if = "is_default")]
            pub blend_mode: BlendMode,
            /// Instructions for agents, read with the node.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub agent_notes: Option<String>,
            /// A link to another frame when the prototype runs.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub prototype: Option<Prototype>,
            /// Free-form JSON for tooling and agents; the editor ignores it.
            #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
            pub metadata: BTreeMap<String, Json>,
            $( $(#[$fmeta])* pub $field: $ty, )*
        }
    };
}

/// A node. The `type` tag says which kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum Node {
    /// A container with optional auto-layout.
    Frame(FrameNode),
    /// A plain container with no layout of its own.
    Group(GroupNode),
    /// A rectangle.
    Rect(RectNode),
    /// An ellipse, ring, arc or pie slice.
    Ellipse(EllipseNode),
    /// A regular polygon.
    Polygon(PolygonNode),
    /// A star.
    Star(StarNode),
    /// A line.
    Line(LineNode),
    /// A path.
    Vector(VectorNode),
    /// A boolean combination of its children.
    Boolean(BooleanNode),
    /// Text.
    Text(TextNode),
    /// An image.
    Image(ImageNode),
    /// An icon from a library.
    Icon(IconNode),
    /// An instance of a component.
    Instance(InstanceNode),
    /// A script whose output renders inside the node.
    Script(ScriptNode),
    /// A sticky note for people.
    Note(NoteNode),
    /// Instructions for agents placed on the canvas.
    Context(ContextNode),
    /// A saved request.
    Prompt(PromptNode),
    /// A live web view (typed now, gated to v1.1).
    Embed(EmbedNode),
}

node_struct!(
    /// A container with optional auto-layout.
    FrameNode {
        /// `none`, `row`, `column` or `grid`.
        #[serde(default, skip_serializing_if = "is_default")]
        layout: Layout,
        /// Grid only: the number of equal columns. ARCHITECTURE gives a grid no track fields; this is
        /// the shape the layout oracle uses, to be settled in Prompt 18.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        columns: Option<u32>,
        /// Grid only: the number of equal rows; as many as the children need when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rows: Option<u32>,
        /// Space between children.
        #[serde(default, skip_serializing_if = "is_default")]
        gap: Value<f64>,
        /// Space inside the edge.
        #[serde(default, skip_serializing_if = "is_default")]
        padding: Padding,
        /// Distribution along the main axis.
        #[serde(default, skip_serializing_if = "is_default")]
        justify: Justify,
        /// Alignment across the main axis.
        #[serde(default, skip_serializing_if = "is_default")]
        align: Align,
        /// Whether the stroke takes part in layout. ARCHITECTURE does not define it; undefined until
        /// Prompt 18.
        #[serde(default, skip_serializing_if = "is_false")]
        layout_includes_stroke: bool,
        /// Clips children to the frame.
        #[serde(default, skip_serializing_if = "is_false")]
        clip: bool,
        /// Rounds the frame's corners (and its clip).
        #[serde(default, skip_serializing_if = "is_default")]
        corner_radius: CornerRadius,
        /// Theme values this subtree uses, by axis (`{"Mode": "Dark"}`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        theme: BTreeMap<String, String>,
        /// Marks the frame as a slot instances can fill.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<Slot>,
        /// Back to front.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<Node>,
    }
);

node_struct!(
    /// A plain container with no layout of its own.
    GroupNode {
        /// Back to front.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<Node>,
    }
);

node_struct!(
    /// A rectangle.
    RectNode {
        /// One radius, or four.
        #[serde(default, skip_serializing_if = "is_default")]
        corner_radius: CornerRadius,
    }
);

node_struct!(
    /// An ellipse, ring, arc or pie slice.
    EllipseNode {
        /// The hole, as a ratio of the radius: 0 is solid, towards 1 is a thin ring.
        #[serde(default, skip_serializing_if = "is_default")]
        inner_radius: f64,
        /// Where the arc starts, in degrees clockwise from the right.
        #[serde(default, skip_serializing_if = "is_default")]
        start_angle: f64,
        /// How far the arc runs, in degrees; 360 is a full ellipse.
        #[serde(default = "full_sweep", skip_serializing_if = "is_full_sweep")]
        sweep_angle: f64,
    }
);

node_struct!(
    /// A regular polygon.
    PolygonNode {
        /// Three or more.
        sides: u32,
        /// Rounds the corners.
        #[serde(default, skip_serializing_if = "is_default")]
        corner_radius: Value<f64>,
    }
);

node_struct!(
    /// A star.
    StarNode {
        /// Two or more.
        points: u32,
        /// Inner radius over outer radius, 0 to 1.
        inner_ratio: f64,
    }
);

node_struct!(
    /// A line. Its length is its width.
    LineNode {}
);

node_struct!(
    /// A path.
    VectorNode {
        /// SVG path data.
        path: String,
        /// The rectangle the path is drawn in; the node's box is that rectangle scaled.
        view_box: ViewBox,
        /// Which regions are inside.
        #[serde(default, skip_serializing_if = "is_default")]
        fill_rule: FillRule,
    }
);

node_struct!(
    /// A boolean combination of its children.
    BooleanNode {
        /// Union, subtract, intersect or exclude.
        op: BoolOp,
        /// Back to front; subtract takes the first away from the rest.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<Node>,
    }
);

node_struct!(
    /// Text.
    TextNode {
        /// Runs in reading order.
        content: Vec<TextRun>,
        /// Horizontal alignment.
        #[serde(default, skip_serializing_if = "is_default")]
        align: TextAlign,
        /// Vertical alignment.
        #[serde(default, skip_serializing_if = "is_default")]
        vertical_align: VerticalAlign,
        /// What changes when the text does not fit.
        #[serde(default, skip_serializing_if = "is_default")]
        resize: TextResize,
        /// A shared text style.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text_style: Option<StyleId>,
    }
);

node_struct!(
    /// An image.
    ImageNode {
        /// The image in the document's assets.
        asset: AssetId,
        /// How the image fills the box.
        #[serde(default, skip_serializing_if = "is_default")]
        fit: ImageFit,
        /// Crops and moves the image.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<Transform>,
    }
);

node_struct!(
    /// An icon from a library.
    IconNode {
        /// The icon library.
        library: String,
        /// The icon's name in that library; a string variable can decide. Called `icon` because `name`
        /// is the layer's own name, which ARCHITECTURE §4.1 also lists as `name`.
        icon: Value<String>,
        /// Stroke weight, 100 to 700.
        weight: u16,
        /// Filled rather than outlined.
        #[serde(default, skip_serializing_if = "is_false")]
        fill: bool,
    }
);

node_struct!(
    /// An instance of a component: the component's tree, with this instance's choices applied.
    InstanceNode {
        /// The component this is an instance of.
        component_id: ComponentId,
        /// The axis values chosen, by axis (`{"State": "hover"}`). The variant whose values equal these applies.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        variant: BTreeMap<String, String>,
        /// The values of the component's props. A prop without one takes its default.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        props: BTreeMap<String, Scalar>,
        /// Changes to nodes inside the component's tree, by id path from its root.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        overrides: BTreeMap<NodeIdPath, Override>,
        /// What fills each of the component's slots, by slot node id.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        slot_content: BTreeMap<NodeId, Vec<Node>>,
    }
);

node_struct!(
    /// A script whose output renders inside the node. The output is derived: rendered, never saved,
    /// not in undo history, so there is no `children` here.
    ScriptNode {
        /// The `.js` file, relative to the document.
        src: String,
        /// Values for the script's declared inputs, by name.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        inputs: BTreeMap<String, Value<Scalar>>,
        /// Clips the output to the node's box.
        #[serde(default, skip_serializing_if = "is_false")]
        clip: bool,
    }
);

node_struct!(
    /// A sticky note for people. Agents can read it.
    NoteNode {
        /// The note text.
        content: String,
        /// The note colour.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<Value<Color>>,
    }
);

node_struct!(
    /// Instructions for agents, placed on the canvas.
    ContextNode {
        /// The instructions.
        content: String,
        /// The node it points at; otherwise it applies to the frame it sits in.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        points_at: Option<NodeId>,
    }
);

node_struct!(
    /// A saved request on the canvas. Run sends it to the agent panel.
    PromptNode {
        /// The request.
        content: String,
        /// Which engine runs it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        engine: Option<String>,
        /// Which model.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    }
);

node_struct!(
    /// A live web view. Typed now, gated to v1.1.
    EmbedNode {
        /// The page to show.
        url: String,
        /// The device frame, by name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device: Option<String>,
        /// 1 is actual size.
        #[serde(default = "crate::defaults::one", skip_serializing_if = "crate::defaults::is_one")]
        zoom: f64,
        /// How far the page is scrolled.
        #[serde(default, skip_serializing_if = "is_default")]
        scroll: Point,
        /// Rounds the view's corners.
        #[serde(default, skip_serializing_if = "is_default")]
        radius: Value<f64>,
    }
);

/// A frame's slot marker.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Slot {
    /// Components that usually go here. Suggestions guide people and agents and never restrict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested: Vec<ComponentId>,
}

/// A prototype link: where to go and how.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Prototype {
    /// The frame to go to.
    pub target: NodeId,
    /// How to get there.
    pub transition: Transition,
}

/// A prototype transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Transition {
    /// How the new frame arrives.
    #[serde(rename = "type")]
    pub kind: TransitionKind,
    /// Length in milliseconds; 0 when absent.
    #[serde(default, skip_serializing_if = "is_default")]
    pub duration_ms: u32,
}

/// Runs `$body` with `$n` bound to the inner struct of whichever kind `$node` is. Every kind has the
/// common fields under the same names, so one body serves them all.
macro_rules! on_each {
    ($node:expr, $n:ident => $body:expr) => {
        match $node {
            Node::Frame($n) => $body,
            Node::Group($n) => $body,
            Node::Rect($n) => $body,
            Node::Ellipse($n) => $body,
            Node::Polygon($n) => $body,
            Node::Star($n) => $body,
            Node::Line($n) => $body,
            Node::Vector($n) => $body,
            Node::Boolean($n) => $body,
            Node::Text($n) => $body,
            Node::Image($n) => $body,
            Node::Icon($n) => $body,
            Node::Instance($n) => $body,
            Node::Script($n) => $body,
            Node::Note($n) => $body,
            Node::Context($n) => $body,
            Node::Prompt($n) => $body,
            Node::Embed($n) => $body,
        }
    };
}

impl Node {
    /// Which kind of node this is.
    pub fn kind(&self) -> NodeKind {
        match self {
            Self::Frame(_) => NodeKind::Frame,
            Self::Group(_) => NodeKind::Group,
            Self::Rect(_) => NodeKind::Rect,
            Self::Ellipse(_) => NodeKind::Ellipse,
            Self::Polygon(_) => NodeKind::Polygon,
            Self::Star(_) => NodeKind::Star,
            Self::Line(_) => NodeKind::Line,
            Self::Vector(_) => NodeKind::Vector,
            Self::Boolean(_) => NodeKind::Boolean,
            Self::Text(_) => NodeKind::Text,
            Self::Image(_) => NodeKind::Image,
            Self::Icon(_) => NodeKind::Icon,
            Self::Instance(_) => NodeKind::Instance,
            Self::Script(_) => NodeKind::Script,
            Self::Note(_) => NodeKind::Note,
            Self::Context(_) => NodeKind::Context,
            Self::Prompt(_) => NodeKind::Prompt,
            Self::Embed(_) => NodeKind::Embed,
        }
    }

    /// The node's id.
    pub fn id(&self) -> &NodeId {
        on_each!(self, n => &n.id)
    }

    /// The node's name.
    pub fn name(&self) -> &str {
        on_each!(self, n => &n.name)
    }

    /// Whether the node is shown.
    pub fn visible(&self) -> &Value<bool> {
        on_each!(self, n => &n.visible)
    }

    /// Whether the node is locked.
    pub fn locked(&self) -> bool {
        on_each!(self, n => n.locked)
    }

    /// The node's opacity.
    pub fn opacity(&self) -> &Value<f64> {
        on_each!(self, n => &n.opacity)
    }

    /// The node's rotation in degrees, clockwise.
    pub fn rotation(&self) -> &Value<f64> {
        on_each!(self, n => &n.rotation)
    }

    /// Mirrored left to right.
    pub fn flip_x(&self) -> bool {
        on_each!(self, n => n.flip_x)
    }

    /// Mirrored top to bottom.
    pub fn flip_y(&self) -> bool {
        on_each!(self, n => n.flip_y)
    }

    /// Where the node sits.
    pub fn position(&self) -> &Position {
        on_each!(self, n => &n.position)
    }

    /// The node's width.
    pub fn width(&self) -> &Size {
        on_each!(self, n => &n.width)
    }

    /// The node's height.
    pub fn height(&self) -> &Size {
        on_each!(self, n => &n.height)
    }

    /// The node's fills, first at the bottom.
    pub fn fills(&self) -> &[Paint] {
        on_each!(self, n => &n.fills)
    }

    /// The node's stroke.
    pub fn stroke(&self) -> Option<&Stroke> {
        on_each!(self, n => n.stroke.as_ref())
    }

    /// The node's effects.
    pub fn effects(&self) -> &[Effect] {
        on_each!(self, n => &n.effects)
    }

    /// How the node blends with what is beneath it.
    pub fn blend_mode(&self) -> BlendMode {
        on_each!(self, n => n.blend_mode)
    }

    /// Notes for agents.
    pub fn agent_notes(&self) -> Option<&str> {
        on_each!(self, n => n.agent_notes.as_deref())
    }

    /// The prototype link.
    pub fn prototype(&self) -> Option<&Prototype> {
        on_each!(self, n => n.prototype.as_ref())
    }

    /// The node's free-form metadata.
    pub fn metadata(&self) -> &BTreeMap<String, Json> {
        on_each!(self, n => &n.metadata)
    }

    /// The node's children, back to front; empty for kinds that have none.
    pub fn children(&self) -> &[Node] {
        match self {
            Self::Frame(n) => &n.children,
            Self::Group(n) => &n.children,
            Self::Boolean(n) => &n.children,
            _ => &[],
        }
    }

    /// The node's children for editing; none for kinds that cannot hold any.
    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Self::Frame(n) => Some(&mut n.children),
            Self::Group(n) => Some(&mut n.children),
            Self::Boolean(n) => Some(&mut n.children),
            _ => None,
        }
    }
}
