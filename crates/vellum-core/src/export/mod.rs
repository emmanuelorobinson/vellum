//! Export plans and bundles shared by tools, clipboard commands and swappable format adapters.
pub mod svg;
use crate::{
    display_list::Output,
    ports::{ExportError, LayoutResult, Rect},
    resolver::ResolvedNode,
};
use std::{collections::BTreeMap, sync::Arc};
use vellum_types::{AssetId, display::DrawCmd};

#[derive(Debug, Clone, PartialEq)]
pub struct ExportFont {
    pub family: String,
    pub weight: u16,
    pub style: vellum_types::FontStyle,
    pub data: crate::ports::FontData,
    /// Document/library fonts travel with editable text. Bundled fonts need only the rendered text layer.
    pub embed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextVisual {
    pub bytes: Vec<u8>,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportFrame {
    pub id: String,
    pub name: String,
    pub bounds: Rect,
    pub scale: f64,
    pub roots: Vec<Arc<ResolvedNode>>,
    pub layout: LayoutResult,
    pub output: Output,
    pub faces: Vec<ExportFont>,
    /// Text pixels from the shared renderer. HTML keeps the source text alongside the visible outlines.
    pub text_visuals: BTreeMap<vellum_types::NodeIdPath, TextVisual>,
    pub images: BTreeMap<AssetId, Arc<[u8]>>,
    pub shaders: BTreeMap<String, Arc<str>>,
    /// The host can require a visual fallback for content it cannot express as DOM.
    pub fallback: bool,
}
impl ExportFrame {
    pub fn empty(id: &str, name: &str, width: f64, height: f64) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            bounds: Rect::new(0., 0., width, height),
            scale: 1.,
            roots: vec![],
            layout: LayoutResult::default(),
            output: Output::default(),
            faces: vec![],
            text_visuals: BTreeMap::new(),
            images: BTreeMap::new(),
            shaders: BTreeMap::new(),
            fallback: false,
        }
    }
    pub fn size(&self) -> (u32, u32) {
        (
            (self.bounds.width * self.scale).ceil() as u32,
            (self.bounds.height * self.scale).ceil() as u32,
        )
    }
    pub fn at_scale(&self, scale: f64) -> Self {
        let mut frame = self.clone();
        let ratio = (scale / self.scale) as f32;
        for cmd in &mut frame.output.list.commands {
            if let DrawCmd::Transform(m) = cmd {
                for v in m {
                    *v *= ratio;
                }
            }
        }
        frame.scale = scale;
        frame
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ExportScene {
    pub frames: Vec<ExportFrame>,
}
impl ExportScene {
    pub fn check(&self, scale: f64) -> Result<(), ExportError> {
        if self.frames.is_empty() || self.frames.len() > 100 {
            return Err(ExportError::new("Select between 1 and 100 layers"));
        }
        let mut pixels = 0.;
        for f in &self.frames {
            let (w, h) = (
                (f.bounds.width * scale).ceil(),
                (f.bounds.height * scale).ceil(),
            );
            if !f.bounds.is_valid()
                || !scale.is_finite()
                || scale <= 0.
                || w < 1.
                || h < 1.
                || w > 8192.
                || h > 8192.
                || w * h > 16_777_216.
            {
                return Err(ExportError::new(
                    "Export exceeds image bounds (8192 px per side, 16.7 megapixels)",
                ));
            }
            if !f.output.problems.is_empty() {
                return Err(ExportError::new(format!(
                    "Incomplete render of {}: {:?}",
                    f.id, f.output.problems
                )));
            }
            pixels += w * h;
        }
        if pixels > 67_108_864. {
            return Err(ExportError::new(
                "Selection exceeds the export pixel budget",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AssetMode {
    #[default]
    Inline,
    Files,
}
#[derive(Debug, Clone)]
pub struct CodeOptions {
    pub scaffold: bool,
    pub assets: AssetMode,
}
impl Default for CodeOptions {
    fn default() -> Self {
        Self {
            scaffold: true,
            assets: AssetMode::Inline,
        }
    }
}
#[derive(Debug, Clone)]
pub struct ImageOptions {
    pub scale: u8,
    pub suffix: String,
}
impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            scale: 1,
            suffix: String::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundle {
    pub files: Vec<ExportFile>,
    pub warnings: Vec<String>,
}
impl ExportBundle {
    pub fn text(&self) -> Option<&str> {
        self.files
            .first()
            .filter(|f| f.mime == "text/html")
            .and_then(|f| std::str::from_utf8(&f.bytes).ok())
    }
    /// Clipboard MIME payloads; hosts pass these bytes to their own clipboard API. File assets must be inlined
    /// before copying. A multi-image ZIP is a download, not an image clipboard item.
    pub fn clipboard(&self) -> Result<BTreeMap<String, Vec<u8>>, ExportError> {
        if self.files.len() != 1 {
            return Err(ExportError::new(
                "Copy requires inline assets and one output",
            ));
        }
        let file = &self.files[0];
        if !matches!(file.mime.as_str(), "image/png" | "text/html") {
            return Err(ExportError::new("Copy supports PNG or HTML"));
        }
        let mut result = BTreeMap::from([(file.mime.clone(), file.bytes.clone())]);
        if file.mime == "text/html" {
            result.insert("text/plain".into(), file.bytes.clone());
        }
        Ok(result)
    }
}
/// Suggested filenames never contain document-supplied path separators or dot traversal. The ordinal makes
/// duplicate names collision-free while preserving selection order.
pub fn file_stem(name: &str, ordinal: usize, suffix: &str) -> String {
    let safe = |s: &str| {
        s.chars()
            .take(80)
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
    };
    let base = safe(name);
    let base = base.trim_matches('-');
    format!(
        "{}-{}{}",
        if base.is_empty() { "frame" } else { base },
        ordinal + 1,
        safe(suffix)
    )
}
