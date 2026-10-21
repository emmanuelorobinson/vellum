//! Independent .pen 2.21 reader, written against public format documentation only.
//! Foreign files are input data, never executable code or Vellum's system of record.
mod convert;
mod graphics;
mod source;
use std::collections::BTreeMap;
use vellum_core::ports::{ImportError, ImportInput, Imported, ImportedAsset, Importer};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_NODES: usize = 20_000;
pub const MAX_DEPTH: usize = 64;
pub type PathBounds = fn(&str) -> Option<[f64; 4]>;
#[derive(Default)]
pub struct PenImporter {
    assets: BTreeMap<String, ImportedAsset>,
    bounds: Option<PathBounds>,
}
impl PenImporter {
    pub fn with_assets(assets: BTreeMap<String, ImportedAsset>) -> Self {
        Self {
            assets,
            bounds: None,
        }
    }
    /// Composition can reuse its existing SVG geometry implementation without adapter dependencies.
    pub fn with_path_bounds(mut self, bounds: PathBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }
    /// Only safe relative image paths. This performs no I/O and never returns script/library/shader URLs.
    pub fn image_paths(input: &ImportInput<'_>) -> Result<Vec<String>, ImportError> {
        let root = source::read(input)?;
        let (_, paths) = convert::run_with_paths(&root, &Self::default())?;
        Ok(paths)
    }
}
impl Importer for PenImporter {
    fn id(&self) -> &'static str {
        "pen"
    }
    fn recognizes(&self, i: &ImportInput<'_>) -> bool {
        i.name.to_ascii_lowercase().ends_with(".pen") || source::read(i).is_ok()
    }
    fn import(&self, i: &ImportInput<'_>) -> Result<Imported, ImportError> {
        convert::run(&source::read(i)?, self)
    }
}
fn error(message: &str) -> ImportError {
    ImportError::new(message)
}
pub fn relative_image(path: &str) -> Option<String> {
    let path = path.strip_prefix("./").unwrap_or(path);
    if path.is_empty()
        || path.len() > 1024
        || path.contains(['\\', ':', '?', '#', '%'])
        || path.chars().any(char::is_control)
        || path.starts_with('/')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        None
    } else {
        Some(path.to_string())
    }
}
