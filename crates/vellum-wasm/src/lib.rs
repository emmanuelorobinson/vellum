//! Composition root for the browser engine.
//!
//! Placeholder: one exported function so the browser target is proven to build. The binding
//! strategy (wasm-bindgen or emscripten exports) is decided by ADR-001 in Prompt 4, so none is
//! chosen here.

// A WebAssembly export needs `#[unsafe(no_mangle)]`, which the workspace-wide `unsafe_code` lint
// denies. This is the one FFI boundary in the crate and contains no unsafe code of its own.
#![allow(unsafe_code)]

/// The `.vellum` format version this engine reads and writes.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_format_version() -> u32 {
    vellum_types::FORMAT_VERSION
}
