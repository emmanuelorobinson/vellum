//! A fake per port: in memory, deterministic, and simple enough to read in one sitting.
//!
//! They exist to unit-test other modules before the real adapters do, and to prove the contract suites accept
//! a correct implementation. They are not meant to be fast or complete: the layout fake lays out rows and
//! columns, the path fake returns its first argument, the script fake runs nothing.

mod assets;
mod clock;
mod codec;
mod content;
mod doc_store;
mod engine;
mod files;
mod fonts;
mod import;
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
mod tokens;
mod watch;

pub use assets::InMemoryAssets;
pub use clock::{ManualClock, SequentialIds};
pub use codec::FakeJsonCodec;
pub use content::{MemorySkills, MemoryStyles};
pub use doc_store::InMemoryDocStore;
pub use engine::{FakeEngine, FakeStep, RecordingBridge, StalledModel};
pub use files::MemFs;
pub use fonts::StaticFonts;
pub use import::{MemoryFetch, MemoryIcons, MemoryImages, MemoryImporter};
pub use layout::StackLayout;
pub use model::{MemorySecrets, ReplayTransport, ScriptedModel};
pub use path::IdentityPathOps;
pub use permissions::ScriptedPrompt;
pub use script::NoopScriptRuntime;
pub use sessions::MemorySessions;
pub use shader::PassthroughShaderCompiler;
pub use spatial::NaiveSpatialIndex;
pub use target::InMemoryTarget;
pub use text::FixedMetricText;
pub use tokens::{MemorySpec, MemoryTokens};
pub use watch::ManualWatcher;
pub mod sync;

mod browser;
pub use browser::MemoryBrowser;
mod media;
pub use media::{
    MemoryFigma, MemoryGenerator, MemoryImageOps, MemoryMediaHttp, MemorySegmentation,
};
