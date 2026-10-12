//! Data types, display list, protocols and brand constants.
//!
//! This crate stays on a dependency allowlist: `serde`, `serde_json`, `schemars` and `ts-rs` (plus
//! `proptest` in tests). Everything the file format, the daemon protocol and the TypeScript side share
//! is defined here, once.
//!
//! Prompt 7 holds the node union and what hangs off a node: paints, strokes, effects, sizing, ids and
//! variable references. Prompt 8 adds variables, themes, components, libraries and the full document.

pub mod brand;
mod color;
mod defaults;
mod document;
mod effect;
mod enums;
mod geometry;
mod ids;
mod node;
mod paint;
mod stroke;
mod text;
mod value;

pub use color::{Color, ColorError};
pub use document::{Document, DocumentMeta};
pub use effect::{Blur, Effect, ShaderEffect, Shadow};
pub use enums::{
    Align, BlendMode, BoolOp, FillRule, FontStyle, GradientKind, ImageFit, Justify, Layout,
    NodeKind, StrokeAlign, StrokeCap, StrokeJoin, TextAlign, TextResize, TransitionKind,
    VerticalAlign,
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
    InstanceNode, LineNode, Node, NoteNode, PolygonNode, PromptNode, Prototype, RectNode,
    ScriptInput, ScriptNode, Slot, StarNode, TextNode, Transition, VectorNode,
};
pub use paint::{
    GradientPaint, GradientStop, ImagePaint, MeshHandles, MeshPaint, MeshPoint, Paint, ShaderPaint,
    SolidPaint, Uniform,
};
pub use stroke::Stroke;
pub use text::TextRun;
pub use value::{EmptyVarRef, Value, VarRef};

/// The `.vellum` file format version this build writes. Older versions stay readable forever;
/// a newer one opens read-only.
pub const FORMAT_VERSION: u32 = 1;
