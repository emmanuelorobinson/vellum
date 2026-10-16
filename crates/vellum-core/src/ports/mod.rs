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
mod content;
mod doc_store;
mod document_files;
mod engine;
mod files;
mod fonts;
mod geometry;
mod layout;
mod model;
mod path;
mod permissions;
mod script;
mod sessions;
mod shader;
mod spatial;
mod target;
mod text;
mod watch;

pub use assets::{AssetCollect, AssetError, AssetHash, AssetHashError, AssetStore, check_mime};
pub use clock::{Clock, IdGen, Timestamp};
pub use codec::{Codec, CodecError, Position};
pub use content::{
    ContentError, SkillGroup, SkillMeta, SkillSource, StyleDensity, StyleLibrary, StyleMeta,
    StyleType, VisualStyle,
};
pub use doc_store::{
    ChangeSet, Commit, DocStore, Kept, Revert, SyncCursor, SyncError, SyncPayload, UndoTurn,
};
pub use document_files::{DocumentFiles, FileError, Loaded, Saved};
pub use engine::{
    AgentEngine, BridgeTool, EngineError, EngineFuture, EngineStatus, McpCall, McpChannel, McpHost,
    ToolBridge, ToolReply, Truncation, TurnEnd, TurnEvent, TurnLimits, TurnRequest, TurnStream,
};
pub use files::{FileSystem, FsError, check_path};
pub use fonts::{FontData, FontHandle, FontSource};
pub use geometry::{Extent, Rect};
pub use layout::{
    Dimension, DirtySet, Insets, LayoutContent, LayoutEngine, LayoutNode, LayoutResult,
    LayoutStyle, LayoutTree,
};
pub use model::*;
pub use path::{Path, PathOps, PathSegment, StrokeStyle};
pub use permissions::{
    Action, Answer, DecisionSource, PermissionBroker, PermissionKind, PermissionPrompt,
    PermissionRecord, PermissionRequest, Question, Refusal,
};
pub use script::{
    ScriptApi, ScriptDiagnostic, ScriptError, ScriptLimits, ScriptOutput, ScriptRuntime,
    ScriptValue,
};
pub use sessions::{
    DocKey, DocKeyError, EventBody, EventSeq, ImageRef, NewEvent, NewSession, SessionEvent,
    SessionKind, SessionMeta, SessionStore, StepOutcome, StepRecord, StoreError, StoredMessage,
    StoredPart, Tokens, TurnTransition, UsageRecord,
};
pub use shader::{CompiledShader, ShaderCompiler, ShaderError, ShaderTarget};
pub use spatial::SpatialIndex;
pub use target::{DocumentTarget, TargetError, TargetMode, TurnUndone};
pub use text::{
    Glyph, GlyphRun, RichText, StyledRun, TextBoxLayout, TextDecoration, TextEngine, TextLayout,
    TextLine, TextMetrics, TextProblem,
};
pub use watch::FileWatcher;
