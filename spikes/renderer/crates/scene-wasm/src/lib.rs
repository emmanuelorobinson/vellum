//! Renderer A's Rust half. Builds the display list and leaves it in linear memory; the TypeScript
//! side reads it through typed-array views and draws with CanvasKit. No wasm-bindgen: plain
//! `extern "C"` exports, so the boundary cost measured is the CanvasKit calls, not glue code.

use std::cell::RefCell;

use spike_scene::{DisplayList, Document, TextEngine, fixture, generate, text_fixture};

#[derive(Default)]
struct Built {
    doc: Option<Document>,
    list: DisplayList,
    strings: Vec<u8>,
    string_index: Vec<u32>,
}

thread_local! {
    static BUILT: RefCell<Built> = RefCell::new(Built::default());
    /// Fixture fonts in `FONT_FILES` order (Prompt 5), and the Parley engine built from them.
    static FONTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    static ENGINE: RefCell<Option<TextEngine>> = const { RefCell::new(None) };
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

/// Scratch memory for the page to copy a font into.
#[unsafe(no_mangle)]
pub extern "C" fn scene_alloc(len: usize) -> *mut u8 {
    let mut bytes = vec![0u8; len].into_boxed_slice();
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    ptr
}

/// Takes ownership of `len` bytes at `ptr` (from `scene_alloc`) as the next fixture font.
///
/// # Safety
/// `ptr` must come from `scene_alloc(len)` and must not be used again by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scene_add_font(ptr: *mut u8, len: usize) {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    FONTS.with_borrow_mut(|fonts| fonts.push(bytes));
    ENGINE.with_borrow_mut(|engine| *engine = None);
}

/// Lays out text fixture `n` with Parley inside this module and builds its display list.
#[unsafe(no_mangle)]
pub extern "C" fn scene_build_text_fixture(n: usize) -> usize {
    let doc = ENGINE.with_borrow_mut(|engine| {
        let engine = engine.get_or_insert_with(|| FONTS.with_borrow(|f| TextEngine::new(f)));
        text_fixture(n, engine)
    });
    keep(doc)
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
