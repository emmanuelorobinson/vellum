//! Import and asset ports (ARCHITECTURE §3.6): [`Importer`], [`IconLibrary`], [`ImageSource`] and the
//! [`HttpFetch`] an image source reaches the network through.
//!
//! Everything here is pure data in, pure data out. An importer is given bytes and returns node specs in the file
//! format plus the asset bytes those nodes name; it never reads a path, fetches a URL or writes a file. The caller
//! (a tool) stores the assets, gives the nodes ids and applies one Op transaction, so imported content is
//! attributed, undoable and diffable like any other change (Invariant 1).
use super::model::SecretString;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// Why content could not be imported. The message is for the person or agent that asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError(pub String);

impl ImportError {
    pub fn new(why: impl fmt::Display) -> Self {
        Self(why.to_string())
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ImportError {}

/// What an importer is given: the bytes, and the file name they came from when there is one (it names the
/// imported layer and can help recognise a format).
#[derive(Debug, Clone, Copy)]
pub struct ImportInput<'a> {
    pub bytes: &'a [u8],
    pub name: &'a str,
}

/// An asset the imported nodes use: the caller stores it and learns its content hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedAsset {
    /// `image/png`, `image/jpeg` or `image/webp`.
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// The text before the asset's position in [`Imported::assets`] in an `asset` field of an imported node.
pub const ASSET_REF_PREFIX: &str = "import:";

/// The value an imported node's `asset` field holds for `Imported::assets[index]`. The caller swaps it for the
/// stored asset's hash, so an importer never needs a hash function.
pub fn asset_ref(index: usize) -> String {
    format!("{ASSET_REF_PREFIX}{index}")
}

/// The position an `asset` value names, when it is an [`asset_ref`].
pub fn asset_ref_index(value: &str) -> Option<usize> {
    value.strip_prefix(ASSET_REF_PREFIX)?.parse().ok()
}

/// What an importer made.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported {
    /// Top-level nodes in the file format (`id` and `name` may be left out; children nest). Positions are
    /// relative to where the caller puts them: a node with an absolute position is placed from the top left of the
    /// imported group (the group's top left is 0, 0); a node without one has none and is placed by its parent.
    pub nodes: Vec<Value>,
    /// Assets the nodes name with [`asset_ref`].
    pub assets: Vec<ImportedAsset>,
    /// What was left out or changed, one plain sentence each ("Figma images are not in a paste").
    pub notes: Vec<String>,
}

impl Imported {
    /// Every asset position the nodes name.
    pub fn used_assets(&self) -> BTreeSet<usize> {
        fn walk(value: &Value, found: &mut BTreeSet<usize>) {
            match value {
                Value::Object(map) => {
                    for (key, inner) in map {
                        if let ("asset", Value::String(text)) = (key.as_str(), inner)
                            && let Some(index) = asset_ref_index(text)
                        {
                            found.insert(index);
                        }
                        walk(inner, found);
                    }
                }
                Value::Array(items) => items.iter().for_each(|item| walk(item, found)),
                _ => {}
            }
        }
        let mut found = BTreeSet::new();
        self.nodes.iter().for_each(|node| walk(node, &mut found));
        found
    }
}

/// Turns foreign content into nodes.
pub trait Importer {
    /// `svg`, `image`, `figma-clipboard`.
    fn id(&self) -> &'static str;
    /// Whether these bytes look like this importer's format. Cheap: it reads the start, not the whole.
    fn recognizes(&self, input: &ImportInput<'_>) -> bool;
    /// Imports `input`. Content this importer cannot read is an error; content it reads but cannot represent
    /// is left out and said in `notes`. Deterministic, and never panics on any input.
    fn import(&self, input: &ImportInput<'_>) -> Result<Imported, ImportError>;
}

/// How an icon is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconStyle {
    /// Lines: the path is stroked, not filled.
    Stroke,
    /// Shapes: the path is filled.
    Fill,
}

/// An icon in a search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconInfo {
    /// The name in its library (`arrow-right`).
    pub name: String,
    /// Words that find it besides its name.
    pub tags: Vec<String>,
}

/// How a stroke-style icon is stroked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconStroke {
    /// In view-box units.
    pub width: f64,
    /// Round caps and joins (the sets use these), rather than butt and miter.
    pub round: bool,
}

/// One icon, ready to become a vector node.
#[derive(Debug, Clone, PartialEq)]
pub struct IconGlyph {
    pub library: String,
    pub name: String,
    /// x, y, width, height of the icon's drawing area.
    pub view_box: [f64; 4],
    /// SVG path data, in view-box units, for the whole icon.
    pub path: String,
    pub style: IconStyle,
    /// The weight actually used: the nearest the library has to the one asked for.
    pub weight: u16,
    /// Whether the filled variant was used. False when the library has none, whatever was asked.
    pub fill: bool,
    /// Present for [`IconStyle::Stroke`].
    pub stroke: Option<IconStroke>,
}

/// A searchable set of icons.
pub trait IconLibrary {
    /// `lucide`, `feather`, `phosphor`, `material-symbols-outlined`, ... The value of an icon node's `library`.
    fn id(&self) -> &str;
    /// The name a person sees.
    fn label(&self) -> &str;
    fn style(&self) -> IconStyle;
    /// The weights the library draws, ascending, each 100 to 700.
    fn weights(&self) -> Vec<u16>;
    /// Whether icons come filled as well as outlined.
    fn supports_fill(&self) -> bool;
    /// How many icons.
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Icons matching every word of `query`, best first, at most `limit`. A word matches a name part, a tag, or the
    /// start of either. The order is the same for the same query. A blank query lists icons by name.
    fn search(&self, query: &str, limit: usize) -> Vec<IconInfo>;
    /// The icon called `name` (exactly) at the weight nearest `weight`, filled when `fill` and the library has it.
    fn glyph(&self, name: &str, weight: u16, fill: bool) -> Option<IconGlyph>;
}

/// What a person or agent asks a stock library for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageQuery {
    pub query: String,
    /// From 1.
    pub page: u32,
    pub per_page: u32,
    pub orientation: Option<Orientation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Landscape,
    Portrait,
}

/// Who made a picture and under what terms: stored in the metadata of every node filled from a stock library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    /// The library: `pixabay`.
    pub provider: String,
    pub author: String,
    pub author_url: String,
    /// The picture's own page.
    pub source_url: String,
    /// The licence's name.
    pub licence: String,
    pub licence_url: String,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageHit {
    /// The provider's id for the picture.
    pub id: String,
    /// What it shows, in the provider's words.
    pub description: String,
    pub width: u32,
    pub height: u32,
    /// A small version to show in a grid.
    pub preview_url: String,
    /// The version that is stored in the repository.
    pub download_url: String,
    pub attribution: Attribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePage {
    pub hits: Vec<ImageHit>,
    /// How many pictures match in all.
    pub total: u64,
}

/// A picture downloaded for storing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchedImage {
    pub mime: String,
    pub bytes: Vec<u8>,
    pub attribution: Attribution,
}

/// Why a stock library did not answer. The messages never contain a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    /// No key is stored for this library.
    NotConfigured,
    /// The library refused the key.
    Authentication,
    RateLimited,
    /// It could not be reached, or did not answer in time.
    Network,
    /// It answered with something that is not a picture or not what was asked for.
    Rejected(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => {
                f.write_str("No stock image key is set. Add one in Settings, Assets.")
            }
            Self::Authentication => f.write_str("The stock image library did not accept the key."),
            Self::RateLimited => {
                f.write_str("The stock image library is rate limiting this key; try again later.")
            }
            Self::Network => f.write_str("The stock image library could not be reached."),
            Self::Rejected(why) => {
                write!(f, "The stock image library's answer was not usable: {why}")
            }
        }
    }
}

impl std::error::Error for ImageError {}

/// A source of pictures for image slots.
pub trait ImageSource {
    /// `stock`.
    fn id(&self) -> &'static str;
    /// The address all of its requests go to, such as `https://pixabay.com`: what a person is asked to allow.
    fn origin(&self) -> &'static str;
    fn search(&self, query: &ImageQuery) -> Result<ImagePage, ImageError>;
    /// Downloads the picture a search result names, once, for storing in the repository. Nothing is ever linked
    /// to the provider's servers afterwards.
    fn fetch(&self, hit: &ImageHit) -> Result<FetchedImage, ImageError>;
}

/// A GET the host performs. The URL may contain `{secret}`, which the transport replaces with the secret just
/// before sending, so a key never sits in a URL that is logged, stored or returned in an error.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub secret: Option<SecretString>,
    /// The most bytes of body to accept.
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchResponse {
    pub status: u16,
    /// The media type without parameters, lower case; empty when the response gave none.
    pub content_type: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchError {
    Network,
    Timeout,
    /// The body was longer than `max_bytes`.
    TooLarge,
    /// The URL is not allowed: not https, has a user name, or points at this computer or a private network.
    Blocked,
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Network => "the network request failed",
            Self::Timeout => "the network request timed out",
            Self::TooLarge => "the answer was larger than allowed",
            Self::Blocked => "the address is not allowed",
        })
    }
}

impl std::error::Error for FetchError {}

/// Plain HTTPS GET. Only an adapter holds an HTTP client (`vellum-model-http`).
pub trait HttpFetch: Send + Sync {
    fn get(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError>;
}
