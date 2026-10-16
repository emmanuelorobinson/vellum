//! Agent instructions and optional visual recipes. Sources are injected; core never discovers files.

use std::collections::BTreeMap;
use std::fmt;

/// The source group shown in the skills menu, in stacking order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillGroup {
    Builtin,
    DesignSystem,
    User,
}

/// A small discovery record. The body is loaded only when requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMeta {
    pub path: String,
    pub name: String,
    pub description: String,
    pub group: SkillGroup,
}

/// Missing, malformed and unavailable content stay distinguishable. Never return a stale body on failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentError {
    NotFound(String),
    Invalid(String),
    Unavailable(String),
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "No content is registered as {p:?}."),
            Self::Invalid(why) => write!(f, "Invalid content: {why}"),
            Self::Unavailable(why) => write!(f, "Content is unavailable: {why}"),
        }
    }
}
impl std::error::Error for ContentError {}

/// Built-in, design-system and explicitly added instructions. Listing is metadata only; errors are explicit.
pub trait SkillSource {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError>;
    fn read(&self, path: &str) -> Result<String, ContentError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleMeta {
    pub name: String,
    pub description: String,
}

/// Font preferences, not an assertion that those fonts are installed.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleType {
    pub body: String,
    pub heading: String,
    pub mono: String,
    pub base_size: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleDensity {
    pub gap: f64,
    pub padding: f64,
    pub control_height: f64,
}

/// An optional canvas recipe, used only when no design system supplies the values.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualStyle {
    pub name: String,
    pub description: String,
    pub palette: BTreeMap<String, String>,
    pub typography: StyleType,
    pub radius: f64,
    pub density: StyleDensity,
}

/// Built-in or explicitly supplied recipes. Names are unique within a library.
pub trait StyleLibrary {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError>;
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError>;
}
