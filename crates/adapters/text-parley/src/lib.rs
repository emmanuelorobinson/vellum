//! Text shaping, bidi and line breaking behind TextEngine. Font bytes are explicit host inputs.
mod document_fonts;
mod fonts;
mod text;
pub use document_fonts::{
    DocumentFonts, FontFormat, FontImport, FontProblem, ImportError, ImportedFont, MAX_FONT_BYTES,
    decode_font, sniff_font,
};
pub use fonts::{FontError, FontTier, ParleyFonts};
pub use text::ParleyText;
/// The Cargo package name, used by the smoke test and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
