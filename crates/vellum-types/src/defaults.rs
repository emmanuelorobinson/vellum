//! Serde helpers. A field at its default is left out when printing, so each value has one spelling
//! (Invariant 5: the file is deterministic).

use crate::value::Value;

pub(crate) fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

pub(crate) fn yes() -> bool {
    true
}

pub(crate) fn is_true(value: &bool) -> bool {
    *value
}

pub(crate) fn visible() -> Value<bool> {
    Value::Literal(true)
}

pub(crate) fn is_visible(value: &Value<bool>) -> bool {
    *value == Value::Literal(true)
}

pub(crate) fn one() -> f64 {
    1.0
}

pub(crate) fn is_one(value: &f64) -> bool {
    *value == 1.0
}

pub(crate) fn opaque() -> Value<f64> {
    Value::Literal(1.0)
}

pub(crate) fn is_opaque(value: &Value<f64>) -> bool {
    *value == Value::Literal(1.0)
}

pub(crate) fn full_sweep() -> f64 {
    360.0
}

pub(crate) fn is_full_sweep(value: &f64) -> bool {
    *value == 360.0
}

pub(crate) fn regular_weight() -> u16 {
    400
}

pub(crate) fn is_regular_weight(value: &u16) -> bool {
    *value == 400
}

/// Implements `Serialize` (as the `Display` text) and `Deserialize` (from a string, through `FromStr`)
/// for a type that is a string in the file. Written out instead of `#[serde(try_from, into)]` because
/// ts-rs warns about those attributes on every build.
macro_rules! serde_as_string {
    ($ty:ty) => {
        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text =
                    <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(deserializer)?;
                text.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

pub(crate) use serde_as_string;
