//! Rich text runs (ARCHITECTURE §4.1, `text`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::defaults::{is_default, is_false, is_regular_weight, regular_weight};
use crate::enums::FontStyle;
use crate::value::Value;

/// A stretch of text with one set of font settings. A text node holds runs in reading order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TextRun {
    /// The characters.
    pub text: String,
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
    /// A link target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}
