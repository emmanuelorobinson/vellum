//! sRGB colour as the file writes it: `#RRGGBB` or `#RRGGBBAA` (guide §9, Colour).

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use ts_rs::TS;

use crate::defaults::serde_as_string;

/// An sRGB colour with alpha. Printed as `#RRGGBB` when opaque and `#RRGGBBAA` otherwise, in upper
/// case; read in either case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, TS)]
#[schemars(with = "String")]
#[ts(export, type = "string")]
pub struct Color {
    /// Red, 0–255.
    pub r: u8,
    /// Green, 0–255.
    pub g: u8,
    /// Blue, 0–255.
    pub b: u8,
    /// Alpha, 0–255; 255 is opaque.
    pub a: u8,
}

impl Color {
    /// An opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// A colour with alpha.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Why a colour string was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorError(String);

impl fmt::Display for ColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is not a colour; write #RRGGBB or #RRGGBBAA",
            self.0
        )
    }
}

impl std::error::Error for ColorError {}

impl FromStr for Color {
    type Err = ColorError;
    fn from_str(text: &str) -> Result<Self, ColorError> {
        let err = || ColorError(text.to_owned());
        let digits = text.strip_prefix('#').ok_or_else(err)?;
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| err());
        let a = if digits.len() == 8 { byte(6)? } else { 255 };
        Ok(Self {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a,
        })
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02X}", self.a)?;
        }
        Ok(())
    }
}

serde_as_string!(Color);
