//! Renderer A's Rust half. Builds the display list and leaves it in linear memory; the TypeScript
//! side reads it through typed-array views and draws with CanvasKit. No wasm-bindgen: plain
//! `extern "C"` exports, so the boundary cost measured is the CanvasKit calls, not glue code.

use std::cell::RefCell;

use spike_scene::{DisplayList, Document, fixture, generate};

#[derive(Default)]
struct Built {
    doc: Option<Document>,
    list: DisplayList,
    strings: Vec<u8>,
    string_index: Vec<u32>,
}

thread_local! {
    static BUILT: RefCell<Built> = RefCell::new(Built::default());
}

fn keep(doc: Document) -> usize {
    let list = DisplayList::build(&doc);
    let (strings, string_index) = list.string_table();
    let count = doc.node_count();
    BUILT.with_borrow_mut(|b| {
        *b = Built {
            doc: Some(doc),
            list,
            strings,
            string_index,
        }
    });
    count
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_build_document(nodes: usize) -> usize {
    keep(generate(nodes))
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_build_fixture(n: usize) -> usize {
    keep(fixture(n))
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_ops_ptr() -> *const f32 {
    BUILT.with_borrow(|b| b.list.ops.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_ops_len() -> usize {
    BUILT.with_borrow(|b| b.list.ops.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_strings_ptr() -> *const u8 {
    BUILT.with_borrow(|b| b.strings.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_strings_len() -> usize {
    BUILT.with_borrow(|b| b.strings.len())
}

/// `[offset, len]` pairs into the strings buffer, one per string id.
#[unsafe(no_mangle)]
pub extern "C" fn scene_string_index_ptr() -> *const u32 {
    BUILT.with_borrow(|b| b.string_index.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_string_count() -> usize {
    BUILT.with_borrow(|b| b.string_index.len() / 2)
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_image_count() -> usize {
    BUILT.with_borrow(|b| b.doc.as_ref().map_or(0, |d| d.images.len()))
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_image_width(i: usize) -> u32 {
    BUILT.with_borrow(|b| b.doc.as_ref().map_or(0, |d| d.images[i].width))
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_image_height(i: usize) -> u32 {
    BUILT.with_borrow(|b| b.doc.as_ref().map_or(0, |d| d.images[i].height))
}

/// Unpremultiplied RGBA8, `width * height * 4` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn scene_image_rgba_ptr(i: usize) -> *const u8 {
    BUILT.with_borrow(|b| {
        b.doc
            .as_ref()
            .map_or(std::ptr::null(), |d| d.images[i].rgba.as_ptr())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_doc_width() -> f32 {
    BUILT.with_borrow(|b| b.doc.as_ref().map_or(0.0, |d| d.width))
}

#[unsafe(no_mangle)]
pub extern "C" fn scene_doc_height() -> f32 {
    BUILT.with_borrow(|b| b.doc.as_ref().map_or(0.0, |d| d.height))
}
