//! The `.vellum` document (ARCHITECTURE §4.1): the meta block and the sections a file holds. Every section is
//! optional except the meta block and the pages. Maps are keyed by id and sorted by it, so the same document
//! always prints the same bytes (Invariant 5). Pages are a list, because their order is part of the design.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::FORMAT_VERSION;
use crate::brand;
use crate::component::Component;
use crate::defaults::is_default;
use crate::enums::{DocumentKind, PageKind};
use crate::ids::{AssetId, ComponentId, PageId, StyleId, VariableId};
use crate::node::Node;
use crate::style::{FontFace, Style};
use crate::variable::Variable;

/// A `.vellum` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Document {
    /// The format version, the name, what made the file and whether it is a library.
    pub meta: DocumentMeta,
    /// The libraries this document uses: alias → relative path of a `*.lib.vellum` file.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub imports: BTreeMap<String, String>,
    /// Theme axes and their values. The first value of each axis is the default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub themes: BTreeMap<String, Vec<String>>,
    /// Variables by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<VariableId, Variable>,
    /// Text and effect styles by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<StyleId, Style>,
    /// Custom fonts, keyed by the asset that holds each font file.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fonts: BTreeMap<AssetId, FontFace>,
    /// Components by id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub components: BTreeMap<ComponentId, Component>,
    /// The pages, in order.
    pub pages: Vec<Page>,
}

/// The meta block of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DocumentMeta {
    /// The `.vellum` format version the file is written in.
    pub format_version: u32,
    /// The document's name.
    pub name: String,
    /// The app and version that created the file, such as `Vellum 0.1.0`.
    pub created_with: String,
    /// A document, or a library other documents import.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: DocumentKind,
}

/// A page: a canvas to draw on, or slides in presenting order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Page {
    /// The page's id.
    pub id: PageId,
    /// The name shown in the page list.
    pub name: String,
    /// A canvas, or slides.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: PageKind,
    /// The page's nodes, back to front.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

impl Document {
    /// An empty document of kind `document` at the current format version, with no pages yet.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            meta: DocumentMeta {
                format_version: FORMAT_VERSION,
                name: name.into(),
                created_with: format!("{} {}", brand::NAME, env!("CARGO_PKG_VERSION")),
                kind: DocumentKind::Document,
            },
            imports: BTreeMap::new(),
            themes: BTreeMap::new(),
            variables: BTreeMap::new(),
            styles: BTreeMap::new(),
            fonts: BTreeMap::new(),
            components: BTreeMap::new(),
            pages: Vec::new(),
        }
    }
}
