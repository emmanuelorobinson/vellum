//! Sizes, positions and the small shapes that several types share.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::value::Value;

/// A point or an offset, in model coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Point {
    /// Horizontal.
    pub x: f64,
    /// Vertical, growing downward.
    pub y: f64,
}

/// The rectangle an SVG path is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ViewBox {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// A 2×3 affine transform `[a, b, c, d, e, f]`, used in the file only for image crops and gradients.
/// The default is the identity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct Transform(pub [f64; 6]);

impl Transform {
    /// Leaves everything where it is.
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One side of a layer's width or height.
///
/// In the file: a number or `{"$var": …}` is a fixed size, `"hug"` and `"fill"` are the keywords, and
/// `{"hug": 40}` / `{"fill": 40}` carry a fallback for when there is nothing to hug or fill. Each value
/// has one spelling. ADR-002 defines what hug, fill and the fallbacks mean.
#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[schemars(with = "SizeRepr")]
#[ts(export, as = "SizeRepr")]
pub enum Size {
    /// A fixed length, or a number variable.
    Fixed(Value<f64>),
    /// As large as the content, or the fallback when there is no content to measure.
    Hug {
        /// Used when hugging has nothing to measure.
        fallback: Option<f64>,
    },
    /// As large as the parent leaves room for, or the fallback when there is no parent to fill.
    Fill {
        /// Used when there is nothing to fill.
        fallback: Option<f64>,
    },
}

/// The spellings of a `Size` in the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum SizeRepr {
    /// `120` or `{"$var": …}`.
    Fixed(Value<f64>),
    /// `"hug"` or `"fill"`.
    Keyword(SizeKeyword),
    /// `{"hug": 40}` or `{"fill": 40}`.
    WithFallback(SizeWithFallback),
}

/// `"hug"` or `"fill"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[schemars(rename_all = "lowercase")]
#[ts(export, rename_all = "lowercase")]
pub enum SizeKeyword {
    /// Size to the content.
    Hug,
    /// Fill the parent.
    Fill,
}

// By hand, for the reason `enums::string_enum` gives: `{"hug": null}` must not read as `"hug"`.
impl Serialize for SizeKeyword {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Hug => "hug",
            Self::Fill => "fill",
        })
    }
}

impl<'de> Deserialize<'de> for SizeKeyword {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let word = <std::borrow::Cow<'de, str> as Deserialize>::deserialize(deserializer)?;
        match &*word {
            "hug" => Ok(Self::Hug),
            "fill" => Ok(Self::Fill),
            other => Err(serde::de::Error::unknown_variant(other, &["hug", "fill"])),
        }
    }
}

/// `{"hug": 40}` or `{"fill": 40}`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SizeWithFallback {
    /// Size to the content, or this when there is none.
    Hug(f64),
    /// Fill the parent, or this when there is none.
    Fill(f64),
}

impl Serialize for Size {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SizeRepr::from(self.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        SizeRepr::deserialize(deserializer).map(Self::from)
    }
}

impl From<SizeRepr> for Size {
    fn from(repr: SizeRepr) -> Self {
        match repr {
            SizeRepr::Fixed(value) => Self::Fixed(value),
            SizeRepr::Keyword(SizeKeyword::Hug) => Self::Hug { fallback: None },
            SizeRepr::Keyword(SizeKeyword::Fill) => Self::Fill { fallback: None },
            SizeRepr::WithFallback(SizeWithFallback::Hug(f)) => Self::Hug { fallback: Some(f) },
            SizeRepr::WithFallback(SizeWithFallback::Fill(f)) => Self::Fill { fallback: Some(f) },
        }
    }
}

impl From<Size> for SizeRepr {
    fn from(size: Size) -> Self {
        match size {
            Size::Fixed(value) => Self::Fixed(value),
            Size::Hug { fallback: None } => Self::Keyword(SizeKeyword::Hug),
            Size::Fill { fallback: None } => Self::Keyword(SizeKeyword::Fill),
            Size::Hug { fallback: Some(f) } => Self::WithFallback(SizeWithFallback::Hug(f)),
            Size::Fill { fallback: Some(f) } => Self::WithFallback(SizeWithFallback::Fill(f)),
        }
    }
}

/// Where a node sits: left to its parent's layout, or at fixed coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum Position {
    /// Placed by the parent's auto-layout; on a page, the origin.
    #[default]
    Auto,
    /// At these coordinates in the parent, outside any auto-layout flow.
    Absolute(Point),
}

/// Padding: one value for all sides, two (vertical, then horizontal, as in CSS) or four (top, right,
/// bottom, left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Padding {
    /// All four sides.
    All(Value<f64>),
    /// Vertical, horizontal.
    Axes([Value<f64>; 2]),
    /// Top, right, bottom, left.
    Sides([Value<f64>; 4]),
}

impl Default for Padding {
    fn default() -> Self {
        Self::All(Value::Literal(0.0))
    }
}

/// Corner radius: one value for every corner, or four (top-left, top-right, bottom-right, bottom-left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum CornerRadius {
    /// Every corner.
    All(Value<f64>),
    /// Top-left, top-right, bottom-right, bottom-left.
    Corners([Value<f64>; 4]),
}

impl Default for CornerRadius {
    fn default() -> Self {
        Self::All(Value::Literal(0.0))
    }
}

/// A stroke's width: one value, or one per side (top, right, bottom, left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum StrokeWidth {
    /// Every side.
    Uniform(Value<f64>),
    /// Top, right, bottom, left.
    PerSide([Value<f64>; 4]),
}
