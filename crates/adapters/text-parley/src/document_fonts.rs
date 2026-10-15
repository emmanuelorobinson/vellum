//! Custom fonts that travel with the document (ARCHITECTURE §4.3): TTF, OTF, WOFF and WOFF2 files stored as
//! assets, unpacked to plain sfnt here, and served by `FontSource` ahead of every other tier.
//!
//! The file in the assets folder is the one the person imported, byte for byte (a WOFF2 stays a WOFF2), so the
//! document is portable and its hash is the file's. Unpacking happens when the font is loaded. Reading files
//! from the disk is the host's business: bytes arrive through an `AssetStore`.
use std::sync::Arc;

use vellum_core::ops::Shell;
use vellum_core::ports::{AssetError, AssetHash, AssetStore, FontHandle};
use vellum_types::{AssetId, Document, FontFace};

use crate::fonts::{FontError, FontTier, ParleyFonts, inspect};

/// The largest font file, and the largest sfnt a web font may say it unpacks to: 128 MiB. A CJK family is about
/// 20 MB; the limit is there so a header claiming gigabytes is refused before anything is allocated.
pub const MAX_FONT_BYTES: usize = 128 * 1024 * 1024;

/// A font file's container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFormat {
    /// TrueType outlines (`0x00010000` or `true`).
    Ttf,
    /// OpenType with CFF outlines (`OTTO`).
    Otf,
    /// A collection of fonts (`ttcf`). Only a collection of one family can be registered.
    Collection,
    /// WOFF 1.
    Woff,
    /// WOFF 2.
    Woff2,
}

impl FontFormat {
    /// The media type an asset of this format is stored with.
    pub fn mime(self) -> &'static str {
        match self {
            Self::Ttf => "font/ttf",
            Self::Otf => "font/otf",
            Self::Collection => "font/collection",
            Self::Woff => "font/woff",
            Self::Woff2 => "font/woff2",
        }
    }
}

/// The container, read from the first four bytes. Not a promise the rest is a font.
pub fn sniff_font(bytes: &[u8]) -> Option<FontFormat> {
    match bytes.get(..4)? {
        [0, 1, 0, 0] | b"true" => Some(FontFormat::Ttf),
        b"OTTO" => Some(FontFormat::Otf),
        b"ttcf" => Some(FontFormat::Collection),
        b"wOFF" => Some(FontFormat::Woff),
        b"wOF2" => Some(FontFormat::Woff2),
        _ => None,
    }
}

/// The plain sfnt bytes of a font file: a TTF, OTF or collection as it is, a WOFF or WOFF2 unpacked.
pub fn decode_font(bytes: &[u8]) -> Result<(FontFormat, Vec<u8>), FontError> {
    let format = sniff_font(bytes).ok_or(FontError::InvalidFont)?;
    if bytes.len() > MAX_FONT_BYTES {
        return Err(FontError::TooLarge);
    }
    let unpacked = match format {
        FontFormat::Woff | FontFormat::Woff2 => {
            // Both headers keep the unpacked size at offset 16 (`totalSfntSize`).
            let declared = bytes
                .get(16..20)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or(FontError::InvalidFont)?;
            if declared as usize > MAX_FONT_BYTES {
                return Err(FontError::TooLarge);
            }
            let result = if format == FontFormat::Woff {
                wuff::decompress_woff1(bytes)
            } else {
                wuff::decompress_woff2(bytes)
            };
            result.map_err(|e| FontError::Unpack(e.to_string()))?
        }
        _ => bytes.to_vec(),
    };
    Ok((format, unpacked))
}

/// A font registered by [`ParleyFonts::import_font`].
#[derive(Debug, Clone)]
pub struct ImportedFont {
    /// What the file was.
    pub format: FontFormat,
    /// Family, weight and style as the font states them.
    pub face: FontFace,
    /// The unpacked font, the bytes `FontSource::data` serves.
    pub sfnt: Arc<[u8]>,
    /// The faces registered.
    pub handles: Vec<FontHandle>,
}

/// A font that could not be loaded from the document's assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontProblem {
    /// The asset the document keeps it under.
    pub asset: AssetId,
    /// The family the document gives it, so the message can name it.
    pub family: String,
    /// What is wrong, in plain words.
    pub reason: String,
}

/// What [`ParleyFonts::load_document_fonts`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocumentFonts {
    /// Fonts now served.
    pub loaded: Vec<AssetId>,
    /// Fonts the document lists that could not be served. Text in those families falls back, and the editor
    /// says so; nothing pretends they loaded.
    pub problems: Vec<FontProblem>,
}

/// What [`ParleyFonts::import_into_document`] decided.
#[derive(Debug, Clone, PartialEq)]
pub struct FontImport {
    /// The document's shell with the font added; apply it with `Op::SetShell`. Unchanged when `already`.
    pub shell: Shell,
    /// The asset holding the file.
    pub asset: AssetId,
    /// How text refers to it.
    pub face: FontFace,
    /// The document already had exactly this file.
    pub already: bool,
}

/// Why a font could not be imported into a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The file is not a usable font.
    Font(FontError),
    /// The assets could not store it.
    Asset(AssetError),
    /// The document already has a different file for this family, weight and style. Remove that one first.
    Duplicate {
        /// The asset already in the document.
        existing: AssetId,
        /// The family both files claim.
        family: String,
    },
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Font(e) => e.fmt(f),
            Self::Asset(e) => e.fmt(f),
            Self::Duplicate { family, .. } => write!(
                f,
                "This document already has a different {family} font file for that weight and style."
            ),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<FontError> for ImportError {
    fn from(e: FontError) -> Self {
        Self::Font(e)
    }
}

impl From<AssetError> for ImportError {
    fn from(e: AssetError) -> Self {
        Self::Asset(e)
    }
}

fn reason(error: &FontError, format: Option<FontFormat>) -> String {
    match error {
        FontError::InvalidFont => "the file is not a font".to_owned(),
        FontError::MultipleFamilies => {
            "the file is a collection of several families; import one family at a time".to_owned()
        }
        FontError::TooLarge => "the font file is too large".to_owned(),
        FontError::Unpack(why) => match format {
            Some(format) => format!("the {format:?} file could not be unpacked: {why}"),
            None => format!("the file could not be unpacked: {why}"),
        },
    }
}

fn same_face(a: &FontFace, b: &FontFace) -> bool {
    a.family.eq_ignore_ascii_case(&b.family) && a.weight == b.weight && a.style == b.style
}

impl ParleyFonts {
    /// Unpacks `bytes` if it is a web font and registers it at `tier`, as the font itself describes it.
    pub fn import_font(&self, tier: FontTier, bytes: &[u8]) -> Result<ImportedFont, FontError> {
        let (format, sfnt) = decode_font(bytes)?;
        let face = inspect(&sfnt)?;
        let handles = self.register_with(tier, sfnt.clone(), None)?;
        Ok(ImportedFont {
            format,
            face,
            sfnt: sfnt.into(),
            handles,
        })
    }

    /// Serves every font `doc` lists, reading each file from `assets` and registering it at the Document tier
    /// under the family, weight and style the document gives it. Loading the same document again adds nothing.
    ///
    /// One registry holds one document's fonts: use one `ParleyFonts` per open document.
    pub fn load_document_fonts(&self, doc: &Document, assets: &dyn AssetStore) -> DocumentFonts {
        let mut report = DocumentFonts::default();
        for (id, face) in &doc.fonts {
            let problem = |reason: String| FontProblem {
                asset: id.clone(),
                family: face.family.clone(),
                reason,
            };
            let already = self
                .state
                .borrow()
                .loaded
                .get(id.as_str())
                .is_some_and(|(loaded, _)| loaded == face);
            if already {
                report.loaded.push(id.clone());
                continue;
            }
            let Ok(hash) = AssetHash::new(id.as_str()) else {
                report.problems.push(problem(
                    "the font's id is not a content hash, so no file can be stored under it".into(),
                ));
                continue;
            };
            let bytes = match assets.get(&hash) {
                Ok(bytes) => bytes,
                Err(AssetError::NotFound(_)) => {
                    report.problems.push(problem(
                        "the font file is missing from the document's assets".into(),
                    ));
                    continue;
                }
                Err(other) => {
                    report.problems.push(problem(other.to_string()));
                    continue;
                }
            };
            let loaded = decode_font(&bytes)
                .and_then(|(_, sfnt)| self.register_with(FontTier::Document, sfnt, Some(face)));
            match loaded {
                Ok(handles) => {
                    self.state
                        .borrow_mut()
                        .loaded
                        .insert(id.to_string(), (face.clone(), handles));
                    report.loaded.push(id.clone());
                }
                Err(error) => report
                    .problems
                    .push(problem(reason(&error, sniff_font(&bytes)))),
            }
        }
        report
    }

    /// Stores a font file in `assets` and returns the shell that lists it in `doc`, after checking it is a
    /// font: a file that is not one never enters the assets. The font is also registered, so it renders at
    /// once. The file is kept exactly as given.
    pub fn import_into_document(
        &self,
        doc: &Document,
        assets: &dyn AssetStore,
        bytes: &[u8],
    ) -> Result<FontImport, ImportError> {
        let (format, sfnt) = decode_font(bytes)?;
        let face = inspect(&sfnt)?;

        let mut already = None;
        for (existing, other) in doc.fonts.iter().filter(|(_, f)| same_face(f, &face)) {
            let same_file = AssetHash::new(existing.as_str())
                .ok()
                .and_then(|hash| assets.get(&hash).ok())
                .is_some_and(|stored| stored == bytes);
            if same_file {
                already = Some(existing.clone());
            } else {
                return Err(ImportError::Duplicate {
                    existing: existing.clone(),
                    family: other.family.clone(),
                });
            }
        }

        let hash = assets.put(bytes, format.mime())?;
        let asset = AssetId::new(hash.as_str()).expect("a hash is a valid asset id");
        let mut shell = Shell::of(doc);
        if already.is_none() {
            shell.fonts.insert(asset.clone(), face.clone());
        }
        let handles = self.register_with(FontTier::Document, sfnt, Some(&face))?;
        self.state
            .borrow_mut()
            .loaded
            .insert(asset.to_string(), (face.clone(), handles));
        Ok(FontImport {
            shell,
            asset,
            face,
            already: already.is_some(),
        })
    }
}
