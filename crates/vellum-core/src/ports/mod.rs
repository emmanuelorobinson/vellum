//! The ports (ARCHITECTURE §3.1): one trait per swappable dependency, with the data types their signatures use.
//!
//! Each port has a fake and a contract suite in `vellum-contracts`. An adapter is not mergeable until it passes
//! the suite, and the suites are generic so every adapter reuses them. Hot paths (layout, spatial index) are
//! meant to be used through generics, cold paths (codec, assets) through `dyn`; every trait here is object safe
//! except where noted.
//!
//! Coordinates are `f64` model units, times are UTC milliseconds (guide §9).

mod assets;
mod clock;
mod codec;
mod doc_store;
mod fonts;
mod geometry;
mod layout;
mod path;
mod script;
mod shader;
mod spatial;
mod text;

pub use assets::{AssetError, AssetHash, AssetHashError, AssetStore};
pub use clock::{Clock, IdGen, Timestamp};
pub use codec::{Codec, CodecError, Position};
pub use doc_store::{ChangeSet, Commit, DocStore, SyncCursor, SyncError, SyncPayload};
pub use fonts::{FontHandle, FontSource};
pub use geometry::{Extent, Rect};
pub use layout::{
    Dimension, DirtySet, Insets, LayoutContent, LayoutEngine, LayoutNode, LayoutResult,
    LayoutStyle, LayoutTree,
};
pub use path::{Path, PathOps, PathSegment, StrokeStyle};
pub use script::{ScriptApi, ScriptError, ScriptLimits, ScriptOutput, ScriptRuntime, ScriptValue};
pub use shader::{CompiledShader, ShaderCompiler, ShaderError, ShaderTarget};
pub use spatial::SpatialIndex;
pub use text::{
    Glyph, GlyphRun, RichText, StyledRun, TextEngine, TextLayout, TextLine, TextMetrics,
};
