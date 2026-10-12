//! Spike A scene data (guide Prompt 4). Spike quality; rewritten in Prompts 22-23.
//!
//! Three things live here, all plain data and all deterministic:
//! - generated documents of 1,000 / 5,000 / 10,000 nodes (frames, text, rects, images),
//! - the 20 non-text pixel fixtures and the 30 text fixtures (Prompt 5, laid out with Parley),
//! - the display list both renderers draw: a flat `f32` command buffer plus a string table and an
//!   image table. Renderer A walks it in TypeScript and calls CanvasKit; Renderer B and the native
//!   PNG path walk it in Rust and call skia-safe.
//!
//! TEST DATA: every document and fixture in this crate is generated test data.

mod display_list;
mod documents;
mod fixtures;
mod text;
mod text_fixtures;

pub use display_list::{DisplayList, blend, op, paint_kind};
pub use documents::{Color, Document, ImageData, Node, Paint, Stroke, generate};
pub use fixtures::{FIXTURE_COUNT, FIXTURE_SIZE, fixture};
pub use text::{FONT_FILES, TextAlign, TextBlock, TextEngine};
pub use text_fixtures::{
    TEXT_FIXTURE_COUNT, TextCategory, TextFixture, text_fixture, text_fixture_spec,
};

/// The colour every renderer clears to before drawing a document (RGB, 0..=255).
pub const CANVAS_BACKGROUND: [u8; 3] = [232, 232, 236];
