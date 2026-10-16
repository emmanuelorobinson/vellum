//! Fonts. Today: bundled, Google Fonts and the project's own folder.

use vellum_types::FontStyle;

/// A font the engine can draw with. Opaque: only the `FontSource` that handed it out knows what it is, and
/// the same family, weight and style always give the same handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FontHandle(pub u64);

/// Immutable bytes and collection face index passed to every renderer. No platform font object leaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontData {
    /// SFNT bytes (TTF/OTF), shared without copying.
    pub bytes: std::sync::Arc<[u8]>,
    /// Face within a font collection, zero for an ordinary font.
    pub index: u32,
}

/// Finds fonts by family, weight and style.
///
/// ARCHITECTURE's sketch takes a family and a style; a weight is part of what a text run asks for, so it is
/// here too.
pub trait FontSource {
    /// The font for this request, or none when the family is unknown. A family that exists but not in this
    /// weight resolves to the nearest weight it has; the caller never gets a different family.
    fn resolve(&self, family: &str, weight: u16, style: FontStyle) -> Option<FontHandle>;
    /// Font bytes for a previously issued handle. Fakes without font bytes return none.
    fn data(&self, _handle: FontHandle) -> Option<FontData> {
        None
    }
    /// The family names this source can draw with, sorted and without repeats (`list_fonts`). A source that
    /// cannot list its families returns none.
    fn families(&self) -> Vec<String> {
        Vec::new()
    }
}
