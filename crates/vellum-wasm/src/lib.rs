//! Composition root for the browser engine.
//!
//! Emscripten composition root for the shared editor renderer (ADR-001, ADR-016).
//! The unknown-unknown target retains only version exports for dependency-free ABI smoke tests.

// A WebAssembly export needs `#[unsafe(no_mangle)]`, which the workspace-wide `unsafe_code` lint
// denies. This is the one FFI boundary in the crate and contains no unsafe code of its own.
#![allow(unsafe_code)]

/// The `.vellum` format version this engine reads and writes.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_format_version() -> u32 {
    vellum_types::FORMAT_VERSION
}

/// Refuse mismatched editor/engine builds before allocating a graphics context.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_display_list_version() -> u32 {
    vellum_types::display::DISPLAY_LIST_VERSION
}
#[cfg(all(feature = "engine", target_os = "emscripten"))]
mod abi;
#[cfg(all(
    feature = "engine",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
pub mod engine;
#[cfg(target_os = "emscripten")]
mod renderer;
