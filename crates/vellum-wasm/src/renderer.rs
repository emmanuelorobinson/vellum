//! Checked ownership boundary: JS may write only buffers allocated by this module. Exports never
//! dereference caller-provided pointers, form borrowed slices from them, or expose Rust addresses.
use std::{cell::RefCell, collections::BTreeMap};
use vellum_core::ports::{FontData, FontHandle};
use vellum_render_skia::{Page, Renderer, Resources, Scene, webgl::WebGl};
use vellum_types::AssetId;
#[derive(Default)]
struct State {
    buffers: BTreeMap<usize, Vec<u8>>,
    resources: Resources,
    renderer: Option<Renderer>,
    gpu: Option<WebGl>,
    scene: Option<Scene>,
    size: (u32, u32),
    error: String,
    zoom: f32,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn status(f: impl FnOnce(&mut State) -> Result<(), String>) -> u32 {
    STATE.with_borrow_mut(|s| match f(s) {
        Ok(()) => 1,
        Err(e) => {
            s.error = e;
            0
        }
    })
}
fn take(s: &mut State, p: usize) -> Result<Vec<u8>, String> {
    s.buffers
        .remove(&p)
        .ok_or_else(|| "Unknown transfer buffer".into())
}
fn buffer(s: &mut State, mut bytes: Vec<u8>) -> usize {
    // Empty Vec pointers alias; reserve one byte so every live allocation has a unique address.
    if bytes.is_empty() {
        bytes.reserve_exact(1);
    }
    let p = bytes.as_mut_ptr() as usize;
    s.buffers.insert(p, bytes);
    p
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_alloc(n: usize) -> usize {
    if n > 64 * 1024 * 1024 {
        return 0;
    }
    STATE.with_borrow_mut(|s| {
        if s.buffers.values().map(|b| b.capacity()).sum::<usize>() + n > 128 * 1024 * 1024 {
            return 0;
        }
        buffer(s, vec![0; n])
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_free(p: usize) {
    STATE.with_borrow_mut(|s| {
        s.buffers.remove(&p);
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_buffer_len(p: usize) -> usize {
    STATE.with_borrow(|s| s.buffers.get(&p).map_or(0, Vec::len))
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_error() -> usize {
    STATE.with_borrow_mut(|s| buffer(s, s.error.as_bytes().to_vec()))
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_resources_clear() {
    STATE.with_borrow_mut(|s| s.resources = Resources::default());
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_font(low: u32, high: u32, index: u32, p: usize) -> u32 {
    status(|s| {
        let bytes = take(s, p)?;
        s.resources.fonts.insert(
            FontHandle(u64::from(low) | (u64::from(high) << 32)),
            FontData {
                bytes: bytes.into(),
                index,
            },
        );
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_image(id: usize, p: usize) -> u32 {
    status(|s| {
        let id = take(s, id)?;
        let bytes = take(s, p)?;
        let id = String::from_utf8(id).map_err(|_| "Invalid image id")?;
        let id = AssetId::new(id).map_err(|_| "Invalid image id")?;
        s.resources.images.insert(id, bytes.into());
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_scene(p: usize) -> u32 {
    status(|s| {
        let bytes = take(s, p)?;
        let renderer = s.renderer.as_ref().ok_or("Renderer is not initialized")?;
        let scene = renderer
            .prepare_buffer(&bytes, &s.resources)
            .map_err(|e| e.to_string())?;
        s.scene = Some(scene);
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_renderer_init(w: u32, h: u32) -> u32 {
    status(|s| {
        let renderer = Renderer::new().map_err(|e| e.to_string())?;
        let gpu = WebGl::new(w, h).map_err(|e| e.to_string())?;
        s.renderer = Some(renderer);
        s.gpu = Some(gpu);
        s.size = (w, h);
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_renderer_resize(w: u32, h: u32) -> u32 {
    status(|s| {
        s.gpu
            .as_mut()
            .ok_or("Renderer context lost")?
            .resize(w, h)
            .map_err(|e| e.to_string())?;
        s.size = (w, h);
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_renderer_lost() {
    STATE.with_borrow_mut(|s| {
        if let Some(g) = s.gpu.as_mut() {
            g.abandon();
        }
        s.gpu = None;
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_render() -> u32 {
    status(|s| {
        s.gpu
            .as_mut()
            .ok_or("Renderer context lost")?
            .draw(s.scene.as_ref(), s.zoom);
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_pixels() -> usize {
    STATE.with_borrow_mut(|s| {
        match s
            .gpu
            .as_mut()
            .ok_or("Renderer context lost".to_string())
            .and_then(|g| g.pixels().map_err(|e| e.to_string()))
        {
            Ok(pixels) => buffer(s, pixels),
            Err(e) => {
                s.error = e;
                0
            }
        }
    })
}
/// CPU path in the same browser module for like-backend goldens.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_raster_pixels() -> usize {
    STATE.with_borrow_mut(|s| {
        let result = s
            .renderer
            .as_ref()
            .zip(s.scene.as_ref())
            .ok_or("Scene not loaded".to_string())
            .and_then(|(r, scene)| {
                r.rgba(&Page {
                    width: s.size.0,
                    height: s.size.1,
                    scene,
                })
                .map_err(|e| e.to_string())
            });
        match result {
            Ok(p) => buffer(s, p),
            Err(e) => {
                s.error = e;
                0
            }
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn vellum_renderer_dispose() {
    STATE.with_borrow_mut(|s| *s = State::default());
}
/// Independent Canvas2D command plan. No WebGL context is needed to compile geometry/fonts.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_basic_plan(p: usize) -> usize {
    STATE.with_borrow_mut(|s| {
        let result = take(s, p).and_then(|bytes| {
            Renderer::new()
                .map_err(|e| e.to_string())?
                .basic_plan(&bytes, &s.resources)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(bytes) => buffer(s, bytes),
            Err(e) => {
                s.error = e;
                0
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vellum_zoom(zoom: f32) -> u32 {
    status(|s| {
        if !zoom.is_finite() || zoom <= 0. {
            return Err("Invalid zoom".into());
        }
        s.zoom = zoom;
        Ok(())
    })
}
