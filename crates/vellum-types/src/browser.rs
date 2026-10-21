//! Browser-owned pixels and computed layout. Browsers never mutate a document.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRequest {
    pub url: String,
    /// Additional origins explicitly authorized by the caller, for assets. No wildcard grants.
    #[serde(default)]
    pub origins: Vec<String>,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub scroll: [f64; 2],
    #[serde(default = "one")]
    pub zoom: f64,
    #[serde(default)]
    pub snapshot: bool,
}
fn one() -> f64 {
    1.
}
impl BrowserRequest {
    pub fn validate(&self) -> Result<(), BrowserError> {
        if !(1..=4096).contains(&self.width)
            || !(1..=4096).contains(&self.height)
            || !self.zoom.is_finite()
            || !(0.1..=4.).contains(&self.zoom)
            || self
                .scroll
                .iter()
                .any(|n| !n.is_finite() || *n < 0. || *n > 1_000_000.)
            || self
                .selector
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 2048)
            || self.url.len() > 8192
            || self.origins.len() > 16
        {
            return Err(BrowserError::InvalidRequest);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomSnapshot {
    pub version: u32,
    pub width: f64,
    pub height: f64,
    /// Paint order; parents precede children. Coordinates are relative to the capture's top left.
    pub nodes: Vec<DomNode>,
    #[serde(default)]
    pub notes: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomNode {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub parent: Option<usize>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub text: Option<String>,
    /// Inlined pixels from a readable image. Never a remote URL.
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub style: BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct BrowserCapture {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub dom: Option<DomSnapshot>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserError {
    InvalidRequest,
    Unavailable,
    Navigation,
    MissingElement,
    TooLarge,
    Timeout,
    Capture,
}
impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRequest => "Use an HTTP(S) address without credentials, a valid selector and a device size of 1 to 4096 pixels.",
            Self::Unavailable => "Chromium is unavailable. Configure VELLUM_CHROME with a browser executable.",
            Self::Navigation => "The page could not be loaded within the authorized origins.",
            Self::MissingElement => "No visible element matches the selector.",
            Self::TooLarge => "The page exceeds the browser capture limits.",
            Self::Timeout => "The browser capture timed out.",
            Self::Capture => "The browser could not capture this page.",
        })
    }
}
impl std::error::Error for BrowserError {}
