//! Renderer-neutral display data. Separate from the document schema; all geometric scalars are f32.
//! Binary ABI and versioning are documented in docs/runbooks/display-list.md.
mod buffer;
use crate::{
    AssetId, BlendMode, BoolOp, FillRule, GradientKind, ImageFit, StrokeAlign, StrokeCap,
    StrokeJoin,
};
pub use buffer::BufferError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;
pub const DISPLAY_LIST_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DisplayList {
    pub commands: Vec<DrawCmd>,
    pub resources: Vec<Resource>,
}
/// Save and Layer each push one state; Restore closes one. Effects configure the enclosing Layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DrawCmd {
    Save,
    Restore,
    /// Sets the absolute local-to-device matrix. Clips already established stay in device space.
    Transform([f32; 6]),
    Clip {
        shape: u32,
    },
    Path {
        shape: u32,
        paint: u32,
        stroke: Option<u32>,
    },
    GlyphRun {
        glyphs: u32,
        paint: u32,
        stroke: Option<u32>,
    },
    Image {
        image: u32,
        dst: [f32; 4],
    },
    Effect {
        effect: u32,
    },
    Layer {
        opacity: f32,
        blend: BlendMode,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Resource {
    Shape(Shape),
    Paint(RenderPaint),
    Stroke(RenderStroke),
    Glyphs(RenderGlyphRun),
    Image(RenderImage),
    Effect(RenderEffect),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Shape {
    Rect {
        size: [f32; 2],
        radii: [f32; 4],
    },
    Ellipse {
        size: [f32; 2],
        inner: f32,
        start: f32,
        sweep: f32,
    },
    Polygon {
        size: [f32; 2],
        sides: u32,
        radius: f32,
    },
    Star {
        size: [f32; 2],
        points: u32,
        inner: f32,
    },
    Line {
        width: f32,
    },
    Vector {
        data: String,
        view_box: [f32; 4],
        size: [f32; 2],
        fill_rule: FillRule,
    },
    Boolean {
        op: BoolOp,
        operands: Vec<ShapeOperand>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ShapeOperand {
    pub transform: [f32; 6],
    pub shape: Shape,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderImage {
    pub asset: AssetId,
    pub fit: ImageFit,
    pub crop: Option<[f32; 6]>,
    pub opacity: f32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum RenderPaint {
    Solid([u8; 4]),
    Gradient {
        kind: GradientKind,
        transform: [f32; 6],
        stops: Vec<RenderStop>,
    },
    Mesh {
        columns: u32,
        rows: u32,
        points: Vec<RenderMeshPoint>,
    },
    Image(RenderImage),
    Shader {
        src: String,
        uniforms: BTreeMap<String, RenderUniform>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderStop {
    pub position: f32,
    pub color: [u8; 4],
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderMeshPoint {
    pub position: [f32; 2],
    pub color: [u8; 4],
    pub handles: Option<[[f32; 2]; 4]>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum RenderUniform {
    Bool(bool),
    Number(f32),
    Color([u8; 4]),
    Vector(Vec<f32>),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderStroke {
    pub widths: [f32; 4],
    pub align: StrokeAlign,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub dash: Vec<f32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum RenderEffect {
    DropShadow(RenderShadow),
    InnerShadow(RenderShadow),
    LayerBlur(f32),
    BackgroundBlur(f32),
    Shader {
        src: String,
        uniforms: BTreeMap<String, RenderUniform>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderShadow {
    pub offset: [f32; 2],
    pub blur: f32,
    pub spread: f32,
    pub color: [u8; 4],
    pub blend: BlendMode,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderGlyphRun {
    /// Opaque exact integer; never cast to f32. Font bytes are supplied separately by FontSource.
    #[ts(type = "bigint")]
    pub font: u64,
    pub size: f32,
    pub glyphs: Vec<RenderGlyph>,
    pub normalized_coords: Vec<i16>,
    pub embolden: bool,
    pub skew: Option<f32>,
    /// Baseline-relative offset and thickness. Glyph positions below already include line offsets.
    pub underline: Option<[f32; 2]>,
    pub strikethrough: Option<[f32; 2]>,
    pub link: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RenderGlyph {
    pub id: u32,
    pub cluster: u32,
    pub position: [f32; 2],
    pub advance: f32,
}
