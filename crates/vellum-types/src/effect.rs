//! Effects (ARCHITECTURE §4.1). Applied in order; each can be hidden.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::color::Color;
use crate::defaults::{is_default, is_true, yes};
use crate::enums::BlendMode;
use crate::paint::Uniform;
use crate::value::Value;

/// An effect. The `type` tag says which.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[ts(export)]
pub enum Effect {
    /// A shadow behind the layer.
    DropShadow(Shadow),
    /// A shadow inside the layer's edge.
    InnerShadow(Shadow),
    /// Blurs the layer.
    LayerBlur(Blur),
    /// Blurs what is behind the layer.
    BackgroundBlur(Blur),
    /// A runtime shader that can sample the layer's own rendering.
    Shader(ShaderEffect),
}

/// The settings of a drop or inner shadow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Shadow {
    /// Horizontal offset.
    pub x: Value<f64>,
    /// Vertical offset.
    pub y: Value<f64>,
    /// Blur radius.
    pub blur: Value<f64>,
    /// How far the shadow grows beyond the shape; 0 when absent.
    #[serde(default, skip_serializing_if = "is_default")]
    pub spread: Value<f64>,
    /// The shadow colour.
    pub color: Value<Color>,
    /// How the shadow combines with what is beneath it.
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend_mode: BlendMode,
    /// False hides the effect without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// The settings of a layer or background blur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Blur {
    /// Blur radius.
    pub radius: Value<f64>,
    /// False hides the effect without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}

/// A shader used as an effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ShaderEffect {
    /// The shader file, relative to the document.
    pub src: String,
    /// Values for the shader's uniforms, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub uniforms: BTreeMap<String, Value<Uniform>>,
    /// False hides the effect without removing it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
}
