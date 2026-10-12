//! Data types, display list, protocols and brand constants.
//!
//! This crate stays on a dependency allowlist: `serde`, `serde_json`, `schemars` and `ts-rs` (plus
//! `proptest` in tests). Everything the file format, the daemon protocol and the TypeScript side share
//! is defined here, once.
//!
//! Prompt 7 holds the node union and what hangs off a node: paints, strokes, effects, sizing, ids and
//! variable references. Prompt 8 holds the document around them: variables, themes, styles, fonts,
//! components with props, variants and overrides, libraries, pages, and the JSON Schema and TypeScript
//! that describe the file.

pub mod brand;
mod color;
mod component;
mod defaults;
mod document;
mod effect;
mod enums;
mod geometry;
mod ids;
mod node;
mod paint;
mod stroke;
mod style;
mod text;
mod value;
mod variable;

pub use color::{Color, ColorError};
pub use component::{
    BooleanProp, Component, NodePatch, NumberProp, Override, PatchError, PropDef, TextProp, Variant,
};
pub use document::{Document, DocumentMeta, Page};
pub use effect::{Blur, Effect, ShaderEffect, Shadow};
pub use enums::{
    Align, BlendMode, BoolOp, DocumentKind, FillRule, FontStyle, GradientKind, ImageFit, Justify,
    Layout, NodeKind, PageKind, StrokeAlign, StrokeCap, StrokeJoin, TextAlign, TextResize,
    TransitionKind, VariableType, VerticalAlign,
};
pub use geometry::{
    CornerRadius, Padding, Point, Position, Size, SizeKeyword, SizeRepr, SizeWithFallback,
    StrokeWidth, Transform, ViewBox,
};
pub use ids::{
    AssetId, ComponentId, IdError, MAX_ID_LEN, NodeId, NodeIdPath, PageId, StyleId, VariableId,
};
pub use node::{
    BooleanNode, ContextNode, EllipseNode, EmbedNode, FrameNode, GroupNode, IconNode, ImageNode,
    InstanceNode, LineNode, NODE_PROPERTIES, Node, NoteNode, PolygonNode, PromptNode, Prototype,
    RectNode, ScriptNode, Slot, StarNode, TextNode, Transition, VectorNode,
};
pub use paint::{
    GradientPaint, GradientStop, ImagePaint, MeshHandles, MeshPaint, MeshPoint, Paint, ShaderPaint,
    SolidPaint, Uniform,
};
pub use stroke::Stroke;
pub use style::{EffectStyle, FontFace, Style, TextStyle};
pub use text::TextRun;
pub use value::{Scalar, Value, VarRef};
pub use variable::{AliasEntry, LiteralEntry, Variable, VariableBody, VariableValue};

/// The `.vellum` file format version this build writes. Older versions stay readable forever;
/// a newer one opens read-only.
pub const FORMAT_VERSION: u32 = 1;
