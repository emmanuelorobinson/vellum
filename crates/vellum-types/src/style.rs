//! Shared styles and fonts (ARCHITECTURE §4.1). A text style holds the settings of a text run without its
//! characters, so a text node can point at one (`textStyle`). An effect style holds effects in order. A
//! font is a custom font file the document carries; the file is keyed by the asset that holds it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::defaults::{is_default, is_false, is_regular_weight, regular_weight};
use crate::effect::Effect;
use crate::enums::FontStyle;
use crate::value::Value;

/// A shared style, listed in the document's `styles` by id. The `type` tag says which kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum Style {
    /// Font, size and spacing for text.
    Text(TextStyle),
    /// Effects for any layer.
    Effect(EffectStyle),
}

/// The settings of a text run, without its characters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TextStyle {
    /// The name shown in the styles list.
    pub name: String,
    /// Font family.
    pub font: String,
    /// 100 to 900; 400 when absent.
    #[serde(default = "regular_weight", skip_serializing_if = "is_regular_weight")]
    pub weight: u16,
    /// Roman or italic.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: FontStyle,
    /// Font size in pixels.
    pub size: Value<f64>,
    /// Line height in pixels; the font's own when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<Value<f64>>,
    /// Extra space between letters in pixels.
    #[serde(default, skip_serializing_if = "is_default")]
    pub letter_spacing: Value<f64>,
    /// Underlined.
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    /// Struck through.
    #[serde(default, skip_serializing_if = "is_false")]
    pub strikethrough: bool,
}

/// Effects applied in order, shared under one name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct EffectStyle {
    /// The name shown in the styles list.
    pub name: String,
    /// The effects, applied in order.
    pub effects: Vec<Effect>,
}

/// A custom font the document carries. The font file is an asset, keyed by its content hash in the
/// document's `fonts`. Text names the family; this says which weight and style the file is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct FontFace {
    /// The family name that text runs use.
    pub family: String,
    /// 100 to 900; 400 when absent.
    #[serde(default = "regular_weight", skip_serializing_if = "is_regular_weight")]
    pub weight: u16,
    /// Roman or italic.
    #[serde(default, skip_serializing_if = "is_default")]
    pub style: FontStyle,
}
