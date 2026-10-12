use vellum_core::ports::{FontHandle, FontSource};
use vellum_types::FontStyle;

/// A fixed list of families. Every weight and style of a family is the same font.
#[derive(Debug, Clone)]
pub struct StaticFonts {
    families: Vec<String>,
}

impl StaticFonts {
    /// These families, matched by exact name.
    pub fn new(families: &[&str]) -> Self {
        Self {
            families: families.iter().map(|f| (*f).to_owned()).collect(),
        }
    }
}

impl FontSource for StaticFonts {
    fn resolve(&self, family: &str, _weight: u16, _style: FontStyle) -> Option<FontHandle> {
        self.families
            .iter()
            .position(|f| f == family)
            .map(|n| FontHandle(n as u64 + 1))
    }
}
