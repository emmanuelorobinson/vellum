//! Deterministic, in-memory exports. Formats never read paths, fetch assets, or write files.
use crate::export::ExportFrame;
use crate::export::{CodeOptions, ExportBundle, ExportScene, ImageOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError(pub String);
impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ExportError {}
impl ExportError {
    pub fn new(why: impl std::fmt::Display) -> Self {
        Self(why.to_string())
    }
}
/// Full-quality drawing supplied by the composition root, shared with the editor. PDF accepts multiple pages;
/// every other encoding takes exactly one. Resources are supplied as bytes; no I/O is hidden here.
pub trait ExportRenderer {
    fn render(&self, frames: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError>;
}
pub trait CodeExporter {
    fn id(&self) -> &'static str;
    fn export(
        &self,
        scene: &ExportScene,
        options: &CodeOptions,
        renderer: &dyn ExportRenderer,
    ) -> Result<ExportBundle, ExportError>;
}
pub trait ImageExporter {
    fn id(&self) -> &'static str;
    fn export(
        &self,
        scene: &ExportScene,
        options: &ImageOptions,
        renderer: &dyn ExportRenderer,
    ) -> Result<ExportBundle, ExportError>;
}
