//! A fake per port: in memory, deterministic, and simple enough to read in one sitting.
//!
//! They exist to unit-test other modules before the real adapters do, and to prove the contract suites accept
//! a correct implementation. They are not meant to be fast or complete: the layout fake lays out rows and
//! columns, the path fake returns its first argument, the script fake runs nothing.

mod assets;
mod clock;
mod codec;
mod doc_store;
mod files;
mod fonts;
mod layout;
mod path;
mod script;
mod shader;
mod spatial;
mod text;

pub use assets::InMemoryAssets;
pub use clock::{ManualClock, SequentialIds};
pub use codec::FakeJsonCodec;
pub use doc_store::InMemoryDocStore;
pub use files::MemFs;
pub use fonts::StaticFonts;
pub use layout::StackLayout;
pub use path::IdentityPathOps;
pub use script::NoopScriptRuntime;
pub use shader::PassthroughShaderCompiler;
pub use spatial::NaiveSpatialIndex;
pub use text::FixedMetricText;
