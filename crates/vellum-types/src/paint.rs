//! Paints: what fills a shape or a stroke (ARCHITECTURE §4.1). Painted in order; each can be hidden.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::color::Color;
use crate::defaults::{is_default, is_opaque, is_true, opaque, yes};
use crate::enums::{GradientKind, ImageFit};
use crate::geometry::{Point, Transform};
use crate::ids::AssetId;
use crate::value::Value;

/// A paint. The `type` tag says which.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum Paint {
    /// One colour.
    Solid(SolidPaint),
    /// A linear, radial or angular gradient.
    Gradient(GradientPaint),
    /// A mesh gradient.
    Mesh(MeshPaint),
    /// An image from the document's assets.
    Image(ImagePaint),
    /// A runtime shader.
    Shader(ShaderPaint),
}

impl Paint {
    /// False when the paint is switched off.
    pub fn visible(&self) -> bool {
        match self {
            Self::Solid(p) => p.visible,
            Self::Gradient(p) => p.visible,
            Self::Mesh(p) => p.visible,
            Self::Image(p) => p.visible,
            Self::Shader(p) => p.visible,
        }
    }
}

/// A single colour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SolidPaint {
    /// The colour, or a colour variable.
    pub color: Value<Color>,
    /// False hides the paint without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// A gradient between colour stops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GradientPaint {
    /// Linear, radial or angular.
    pub kind: GradientKind,
    /// Colour stops, by position from 0 to 1.
    pub stops: Vec<GradientStop>,
    /// Maps the unit gradient onto the shape; the identity when absent.
    #[serde(default, skip_serializing_if = "is_default")]
    pub transform: Transform,
    /// False hides the paint without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// One stop of a gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GradientStop {
    /// The colour at this stop.
    pub color: Value<Color>,
    /// Where the stop sits, from 0 to 1.
    pub position: f64,
}

/// A mesh gradient: `columns` × `rows` points in row-major order, each with a colour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshPaint {
    /// Points per row.
    pub columns: u32,
    /// Points per column.
    pub rows: u32,
    /// `columns × rows` points, row by row.
    pub points: Vec<MeshPoint>,
    /// False hides the paint without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// A point of a mesh gradient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeshPoint {
    /// Position across the shape, 0 to 1.
    pub x: f64,
    /// Position down the shape, 0 to 1.
    pub y: f64,
    /// The colour at this point.
    pub color: Value<Color>,
    /// Bezier handles that bend the mesh lines through this point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handles: Option<MeshHandles>,
}

/// The four handles of a mesh point, as offsets from the point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct MeshHandles {
    /// Towards the previous column.
    pub left: Point,
    /// Towards the next column.
    pub right: Point,
    /// Towards the previous row.
    pub up: Point,
    /// Towards the next row.
    pub down: Point,
}

/// An image used as a paint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ImagePaint {
    /// The image in the document's assets.
    pub asset: AssetId,
    /// How the image fills the shape.
    #[serde(default, skip_serializing_if = "is_default")]
    pub fit: ImageFit,
    /// Crops and moves the image; absent means uncropped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Transform>,
    /// 0 to 1.
    #[serde(default = "opaque", skip_serializing_if = "is_opaque")]
    pub opacity: Value<f64>,
    /// False hides the paint without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// A runtime shader used as a paint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ShaderPaint {
    /// The shader file, relative to the document.
    pub src: String,
    /// Values for the shader's uniforms, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub uniforms: BTreeMap<String, Value<Uniform>>,
    /// False hides the paint without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// A shader uniform's value: a number, boolean, colour or a vector of two to four numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Uniform {
    /// `true` or `false`.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A colour string.
    Color(Color),
    /// Two numbers.
    Vec2([f64; 2]),
    /// Three numbers.
    Vec3([f64; 3]),
    /// Four numbers.
    Vec4([f64; 4]),
}
