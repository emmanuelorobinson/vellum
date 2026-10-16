//! Explicit font inputs. No filesystem discovery, download or platform font handles.
use parley::{
    FontContext,
    fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, FontWeight, FontWidth},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use vellum_core::ports::{FontData, FontHandle, FontSource};
use vellum_types::{FontFace, FontStyle};

/// Family lookup priority. A higher-priority family owns style/weight matching, even if a lower
/// tier has an exact weight. GoogleCache and System describe bytes supplied by the composition root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontTier {
    Document,
    GoogleCache,
    System,
    Bundled,
}

/// Registration failed without changing the available fonts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    InvalidFont,
    MultipleFamilies,
    /// The file, or the size it says it unpacks to, is past [`crate::MAX_FONT_BYTES`].
    TooLarge,
    /// A WOFF or WOFF2 file that does not unpack, with the decoder's reason.
    Unpack(String),
}
impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::InvalidFont => "invalid font; supply decoded TTF or OTF bytes",
                Self::MultipleFamilies =>
                    "split multi-family font collections into individual fonts before registration",
                Self::TooLarge => "the font file is too large",
                Self::Unpack(reason) =>
                    return write!(f, "the web font could not be unpacked: {reason}"),
            }
        )
    }
}
impl std::error::Error for FontError {}

pub(crate) struct Family {
    pub name: String,
    pub alias: String,
    pub tier: FontTier,
}
pub(crate) struct Face {
    pub handle: FontHandle,
    pub blob_id: u64,
    pub data: FontData,
}
pub(crate) struct State {
    pub cx: FontContext,
    pub families: Vec<Family>,
    pub faces: Vec<Face>,
    /// Document fonts already registered, by asset id, so loading a document twice adds nothing.
    pub loaded: std::collections::BTreeMap<String, (FontFace, Vec<FontHandle>)>,
}
fn collection() -> Collection {
    Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    })
}

/// What a font file says about itself.
pub(crate) fn inspect(sfnt: &[u8]) -> Result<FontFace, FontError> {
    let mut probe = collection();
    let found = probe.register_fonts(Blob::new(Arc::new(sfnt.to_vec())), None);
    if found.len() != 1 {
        return Err(if found.is_empty() {
            FontError::InvalidFont
        } else {
            FontError::MultipleFamilies
        });
    }
    let family = probe
        .family_name(found[0].0)
        .ok_or(FontError::InvalidFont)?
        .to_owned();
    let info = found[0].1.first().ok_or(FontError::InvalidFont)?;
    let style = match info.style() {
        parley::FontStyle::Normal => FontStyle::Normal,
        _ => FontStyle::Italic,
    };
    Ok(FontFace {
        family,
        weight: info.weight().value().round().clamp(100.0, 900.0) as u16,
        style,
    })
}

/// Shared font registry for shaping and drawing. Old handles remain readable after new registrations.
/// Clones share one registry on the owning thread; create a registry per independent worker.
#[derive(Clone)]
pub struct ParleyFonts {
    pub(crate) state: Rc<RefCell<State>>,
}
impl Default for ParleyFonts {
    fn default() -> Self {
        Self::new()
    }
}
impl ParleyFonts {
    /// Available offline on every host; no implicit system enumeration or network access.
    pub fn new() -> Self {
        let fonts = Self {
            state: Rc::new(RefCell::new(State {
                cx: FontContext {
                    collection: collection(),
                    source_cache: Default::default(),
                },
                families: vec![],
                faces: vec![],
                loaded: Default::default(),
            })),
        };
        fonts
            .register(
                FontTier::Bundled,
                include_bytes!("../fonts/InterVariable.ttf").to_vec(),
            )
            .expect("bundled Inter is valid");
        fonts
            .register(
                FontTier::Bundled,
                include_bytes!("../fonts/NotoColorEmoji.ttf").to_vec(),
            )
            .expect("bundled emoji is valid");
        fonts
    }
    /// Register a single-family SFNT (all weights in a collection are allowed). Metadata comes from
    /// the font. Browser WOFF/WOFF2 decoding and reading document/cache/system files belong to hosts.
    /// Same-tier later faces replace matching attributes for new layouts; issued handles stay valid.
    pub fn register(&self, tier: FontTier, bytes: Vec<u8>) -> Result<Vec<FontHandle>, FontError> {
        self.register_with(tier, bytes, None)
    }
    /// Like [`Self::register`], but `face` (a document's `FontFace`) says what family, weight and style the
    /// file answers to, whatever it calls itself. The document is the authority on what its text runs ask for.
    pub(crate) fn register_with(
        &self,
        tier: FontTier,
        bytes: Vec<u8>,
        face: Option<&FontFace>,
    ) -> Result<Vec<FontHandle>, FontError> {
        let bytes: Arc<[u8]> = bytes.into();
        let blob = Blob::new(Arc::new(bytes.clone()));
        let mut probe = collection();
        let found = probe.register_fonts(blob.clone(), None);
        if found.is_empty() {
            return Err(FontError::InvalidFont);
        }
        if found.len() != 1 {
            return Err(FontError::MultipleFamilies);
        }
        let name = match face {
            Some(face) => face.family.clone(),
            None => probe
                .family_name(found[0].0)
                .ok_or(FontError::InvalidFont)?
                .to_owned(),
        };
        let mut state = self.state.borrow_mut();
        let alias = state
            .families
            .iter()
            .find(|f| f.tier == tier && f.name.eq_ignore_ascii_case(&name))
            .map(|f| f.alias.clone())
            .unwrap_or_else(|| format!("__text_family_{}", state.families.len()));
        let registered = state.cx.collection.register_fonts(
            blob.clone(),
            Some(FontInfoOverride {
                family_name: Some(&alias),
                weight: face.map(|f| FontWeight::new(f.weight as f32)),
                style: face.map(|f| to_style(f.style)),
                ..Default::default()
            }),
        );
        if registered.is_empty() {
            return Err(FontError::InvalidFont);
        }
        if !state.families.iter().any(|f| f.alias == alias) {
            state.families.push(Family { name, alias, tier });
        }
        let mut handles = Vec::new();
        for (_, infos) in registered {
            for info in infos {
                let handle = FontHandle(state.faces.len() as u64);
                state.faces.push(Face {
                    handle,
                    blob_id: blob.id(),
                    data: FontData {
                        bytes: bytes.clone(),
                        index: info.index(),
                    },
                });
                handles.push(handle);
            }
        }
        Ok(handles)
    }
    /// The named family in the highest available tier, without substitution.
    pub(crate) fn alias(state: &State, family: &str) -> Option<String> {
        state
            .families
            .iter()
            .filter(|f| f.name.eq_ignore_ascii_case(family))
            .min_by_key(|f| f.tier)
            .map(|f| f.alias.clone())
    }
}
impl FontSource for ParleyFonts {
    fn resolve(&self, family: &str, weight: u16, style: FontStyle) -> Option<FontHandle> {
        let mut state = self.state.borrow_mut();
        let alias = Self::alias(&state, family)?;
        let family = state.cx.collection.family_by_name(&alias)?;
        let info = family.match_font(
            FontWidth::NORMAL,
            to_style(style),
            FontWeight::new(weight as f32),
            true,
        )?;
        let blob = info.load(Some(&mut state.cx.source_cache))?;
        state
            .faces
            .iter()
            .find(|f| f.blob_id == blob.id() && f.data.index == info.index())
            .map(|f| f.handle)
    }
    fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .state
            .borrow()
            .families
            .iter()
            .map(|f| f.name.clone())
            .collect();
        names.sort_by_key(|name| name.to_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        names
    }
    fn data(&self, handle: FontHandle) -> Option<FontData> {
        self.state
            .borrow()
            .faces
            .iter()
            .find(|f| f.handle == handle)
            .map(|f| f.data.clone())
    }
}
pub(crate) fn to_style(style: FontStyle) -> parley::FontStyle {
    match style {
        FontStyle::Normal => parley::FontStyle::Normal,
        FontStyle::Italic => parley::FontStyle::Italic,
    }
}
