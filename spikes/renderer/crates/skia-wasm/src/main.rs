//! Renderer B: skia-safe compiled to WASM (emscripten), drawing into WebGL2 from Rust.
//!
//! The page creates a WebGL2 context with emscripten's `GL.createContext`, makes it current, then
//! calls `init`. From there every draw call is Rust -> Skia inside one WASM module; JS only passes
//! the view transform once per frame.

use std::cell::RefCell;

use spike_scene::{
    CANVAS_BACKGROUND, DisplayList, Document, TextEngine, fixture, generate, text_fixture,
};
use spike_skia_draw::skia_safe::{
    AlphaType, Color, ColorType, ImageInfo, Surface,
    gpu::{
        self, DirectContext, SurfaceOrigin, backend_render_targets, direct_contexts,
        gl::FramebufferInfo,
    },
    surfaces,
};
use spike_skia_draw::{SkiaScene, make_image, make_typeface};

const GL_RGBA8: u32 = 0x8058;

struct State {
    context: DirectContext,
    surface: Surface,
    scene: Option<SkiaScene>,
    font: Vec<u8>,
    node_count: usize,
    doc_size: (f32, f32),
    raster_pixels: Vec<u8>,
    /// Fixture fonts in `FONT_FILES` order (Prompt 5), the Parley engine and the Skia typefaces.
    glyph_fonts: Vec<Vec<u8>>,
    engine: Option<TextEngine>,
    glyph_typefaces: Vec<spike_skia_draw::skia_safe::Typeface>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn make_surface(context: &mut DirectContext, width: i32, height: i32) -> Surface {
    let info = FramebufferInfo {
        fboid: 0,
        format: GL_RGBA8,
        ..Default::default()
    };
    let target = backend_render_targets::make_gl((width, height), 0, 8, info);
    gpu::surfaces::wrap_backend_render_target(
        context,
        &target,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        None,
        None,
    )
    .expect("wrap the WebGL default framebuffer")
}

fn load(doc: Document) {
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("init first");
        let images = doc
            .images
            .iter()
            .map(|data| {
                let raster = make_image(data);
                // Upload once, so frames draw textures instead of re-uploading pixels.
                gpu::images::texture_from_image(
                    &mut state.context,
                    &raster,
                    gpu::Mipmapped::No,
                    gpu::Budgeted::Yes,
                )
                .unwrap_or(raster)
            })
            .collect();
        let typeface = if state.font.is_empty() {
            None
        } else {
            make_typeface(&state.font)
        };
        state.node_count = doc.node_count();
        state.doc_size = (doc.width, doc.height);
        state.scene = Some(
            SkiaScene::new(DisplayList::build(&doc), images, typeface)
                .with_glyph_typefaces(state.glyph_typefaces.clone()),
        );
    });
}

/// Scratch memory for the page to copy bytes (the font) into.
#[unsafe(no_mangle)]
pub extern "C" fn spike_alloc(len: usize) -> *mut u8 {
    let mut bytes = vec![0u8; len].into_boxed_slice();
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_init(width: i32, height: i32) -> i32 {
    let Some(interface) = gpu::gl::Interface::new_native() else {
        return 1;
    };
    let Some(mut context) = direct_contexts::make_gl(interface, None) else {
        return 2;
    };
    let surface = make_surface(&mut context, width, height);
    STATE.with_borrow_mut(|state| {
        *state = Some(State {
            context,
            surface,
            scene: None,
            font: Vec::new(),
            node_count: 0,
            doc_size: (0.0, 0.0),
            raster_pixels: Vec::new(),
            glyph_fonts: Vec::new(),
            engine: None,
            glyph_typefaces: Vec::new(),
        })
    });
    0
}

/// Takes ownership of `len` bytes at `ptr` (from `spike_alloc`) as the text font.
///
/// # Safety
/// `ptr` must come from `spike_alloc(len)` and must not be used again by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spike_set_font(ptr: *mut u8, len: usize) {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    STATE.with_borrow_mut(|state| state.as_mut().expect("init first").font = bytes);
}

/// Takes ownership of `len` bytes at `ptr` (from `spike_alloc`) as the next fixture font.
///
/// # Safety
/// `ptr` must come from `spike_alloc(len)` and must not be used again by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spike_add_glyph_font(ptr: *mut u8, len: usize) -> i32 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len) };
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("init first");
        let Some(typeface) = make_typeface(&bytes) else {
            return 1;
        };
        state.glyph_typefaces.push(typeface);
        state.glyph_fonts.push(bytes);
        state.engine = None;
        0
    })
}

/// Lays out text fixture `n` with Parley inside this module and loads it.
#[unsafe(no_mangle)]
pub extern "C" fn spike_load_text_fixture(n: usize) -> usize {
    let doc = STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("init first");
        let fonts = &state.glyph_fonts;
        let engine = state.engine.get_or_insert_with(|| TextEngine::new(fonts));
        text_fixture(n, engine)
    });
    load(doc);
    spike_node_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_resize(width: i32, height: i32) {
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("init first");
        state.surface = make_surface(&mut state.context, width, height);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_load_document(nodes: usize) -> usize {
    load(generate(nodes));
    spike_node_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_load_fixture(n: usize) -> usize {
    load(fixture(n));
    spike_node_count()
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_node_count() -> usize {
    STATE.with_borrow(|state| state.as_ref().map_or(0, |s| s.node_count))
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_doc_width() -> f32 {
    STATE.with_borrow(|state| state.as_ref().map_or(0.0, |s| s.doc_size.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn spike_doc_height() -> f32 {
    STATE.with_borrow(|state| state.as_ref().map_or(0.0, |s| s.doc_size.1))
}

/// Draws one frame with the view transform (device px = doc px * scale + translate) and submits it.
#[unsafe(no_mangle)]
pub extern "C" fn spike_render(tx: f32, ty: f32, scale: f32) {
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("init first");
        let canvas = state.surface.canvas();
        let [r, g, b] = CANVAS_BACKGROUND;
        canvas.clear(Color::from_rgb(r, g, b));
        canvas.save();
        canvas.translate((tx, ty));
        canvas.scale((scale, scale));
        if let Some(scene) = state.scene.as_mut() {
            scene.draw(canvas);
        }
        canvas.restore();
        state.context.flush_and_submit();
    });
}

fn main() {}

/// Draws the loaded scene on a CPU raster surface inside WASM (no GPU) and returns a pointer to
/// `width * height * 4` bytes of unpremultiplied RGBA. Separates "WASM vs native" from "GPU vs CPU".
#[unsafe(no_mangle)]
pub extern "C" fn spike_render_raster(width: i32, height: i32) -> *const u8 {
    STATE.with_borrow_mut(|state| {
        let state = state.as_mut().expect("init first");
        let mut surface = surfaces::raster_n32_premul((width, height)).expect("raster surface");
        let canvas = surface.canvas();
        let [r, g, b] = CANVAS_BACKGROUND;
        canvas.clear(Color::from_rgb(r, g, b));
        if let Some(scene) = state.scene.as_mut() {
            scene.draw(canvas);
        }
        let info = ImageInfo::new(
            (width, height),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        state.raster_pixels = vec![0; (width * height * 4) as usize];
        surface.read_pixels(&info, &mut state.raster_pixels, width as usize * 4, (0, 0));
        state.raster_pixels.as_ptr()
    })
}

/// Size measurement only: lays out every string of the loaded scene with Skia's paragraph module, so
/// the `textlayout` build keeps SkParagraph, SkShaper, HarfBuzz and ICU instead of stripping them.
#[cfg(feature = "textlayout")]
#[unsafe(no_mangle)]
pub extern "C" fn spike_paragraph_width() -> f32 {
    use spike_skia_draw::skia_safe::textlayout::{
        FontCollection, ParagraphBuilder, ParagraphStyle, TextStyle, TypefaceFontProvider,
    };
    STATE.with_borrow(|state| {
        let state = state.as_ref().expect("init first");
        let Some(typeface) = make_typeface(&state.font) else {
            return 0.0;
        };
        let mut provider = TypefaceFontProvider::new();
        provider.register_typeface(typeface, Some("spike"));
        let mut fonts = FontCollection::new();
        fonts.set_default_font_manager(Some(provider.into()), "spike");
        let mut style = TextStyle::new();
        style.set_font_families(&["spike"]);
        let mut builder = ParagraphBuilder::new(&ParagraphStyle::new(), fonts);
        builder.push_style(&style);
        for s in state
            .scene
            .iter()
            .flat_map(|scene| scene.list.strings.iter())
        {
            builder.add_text(s);
        }
        let mut paragraph = builder.build();
        paragraph.layout(1000.0);
        paragraph.max_intrinsic_width()
    })
}

/// Size measurement only: evaluates a script with QuickJS so the `quickjs` build keeps the engine.
/// Returns the script's numeric result.
#[cfg(feature = "quickjs")]
#[unsafe(no_mangle)]
pub extern "C" fn spike_quickjs_eval_sum(n: i32) -> f64 {
    let runtime = rquickjs::Runtime::new().expect("runtime");
    runtime.set_max_stack_size(256 * 1024);
    runtime.set_memory_limit(64 * 1024 * 1024);
    let context = rquickjs::Context::full(&runtime).expect("context");
    context.with(|ctx| {
        ctx.eval::<f64, _>(format!(
            "let s = 0; for (let i = 0; i < {n}; i++) s += i; s"
        ))
        .unwrap_or(-1.0)
    })
}
