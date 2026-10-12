//! Strokes (ARCHITECTURE §4.1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::defaults::is_default;
use crate::enums::{StrokeAlign, StrokeCap, StrokeJoin};
use crate::geometry::StrokeWidth;
use crate::paint::Paint;

/// The outline of a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Stroke {
    /// Painted in order.
    pub paints: Vec<Paint>,
    /// One width, or one per side.
    pub width: StrokeWidth,
    /// Inside, centred on or outside the edge.
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: StrokeAlign,
    /// The end of an open stroke.
    #[serde(default, skip_serializing_if = "is_default")]
    pub cap: StrokeCap,
    /// The corner of a stroke.
    #[serde(default, skip_serializing_if = "is_default")]
    pub join: StrokeJoin,
    /// Alternating dash and gap lengths; empty means solid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dash: Vec<f64>,
}
