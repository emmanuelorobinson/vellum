//! Bounded insertion-order caches, keyed by exact frame content, assets and device size.
use crate::{Error, Resources, Scene, matrix, raster};
use skia_safe::{self as sk, Canvas, Image, Paint, Picture, Rect};
use std::cell::RefCell;
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheStats {
    pub pictures: usize,
    pub picture_hits: usize,
    pub tile_builds: usize,
    pub tile_hits: usize,
    pub tile_bytes: usize,
}
#[derive(Default)]
pub(crate) struct DrawCache {
    picture: Option<((i32, i32), Picture)>,
    tiles: Vec<(Image, Rect)>,
    pub stats: CacheStats,
}
pub(crate) struct FrameEntry {
    pub id: String,
    pub bytes: Vec<u8>,
    pub resources: Resources,
    pub scene: Scene,
}
pub(crate) fn resource_bytes(r: &Resources) -> usize {
    r.shaders.values().map(|s| s.len()).sum::<usize>()
        + r.images.values().map(|v| v.len()).sum::<usize>()
        + r.fonts.values().map(|v| v.bytes.len()).sum::<usize>()
}
pub(crate) fn equal(a: &Resources, b: &Resources) -> bool {
    a.shaders == b.shaders
        && a.images == b.images
        && a.fonts.len() == b.fonts.len()
        && a.fonts.iter().all(|(id, f)| {
            b.fonts
                .get(id)
                .is_some_and(|g| f.index == g.index && f.bytes == g.bytes)
        })
}
pub(crate) fn draw(scene: &Scene, canvas: &Canvas, zoom: f32, cache: &RefCell<DrawCache>) {
    let size = canvas.base_layer_size();
    let key = (size.width, size.height);
    let mut cache = cache.borrow_mut();
    if cache.picture.as_ref().is_none_or(|(k, _)| *k != key) {
        let mut rec = sk::PictureRecorder::new();
        let c = rec.begin_recording(Rect::from_wh(size.width as f32, size.height as f32), true);
        scene.draw_ops(c);
        cache.picture = rec.finish_recording_as_picture(None).map(|p| (key, p));
        cache.tiles.clear();
        cache.stats.tile_bytes = 0;
        cache.stats.pictures += 1;
    } else {
        cache.stats.picture_hits += 1;
    }
    let Some((_, picture)) = &cache.picture else {
        return;
    };
    let picture = picture.clone();
    // Only whole, isolated viewport scenes are tiled; backdrop-dependent subframes retain pictures.
    let filtered = scene.ops.iter().any(|op| match op {
        crate::Op::Layer { paint, backdrop } => {
            paint.image_filter().is_some() || backdrop.is_some()
        }
        _ => false,
    });
    if filtered
        || (zoom.is_finite()
            && zoom > 0.
            && zoom < 0.5
            && size.width as u64 * size.height as u64 * 4 <= 16 * 1024 * 1024)
    {
        if cache.tiles.is_empty() {
            if let Ok(mut s) = raster(size.width as u32, size.height as u32) {
                s.canvas().draw_picture(&picture, None, None);
                for y in (0..size.height).step_by(512) {
                    for x in (0..size.width).step_by(512) {
                        let r = sk::IRect::from_xywh(
                            x,
                            y,
                            512.min(size.width - x),
                            512.min(size.height - y),
                        );
                        if let Some(image) = s.image_snapshot_with_bounds(r) {
                            cache.tiles.push((
                                image,
                                Rect::from_xywh(
                                    x as f32,
                                    y as f32,
                                    r.width() as f32,
                                    r.height() as f32,
                                ),
                            ));
                        }
                    }
                }
                cache.stats.tile_builds += 1;
                cache.stats.tile_bytes = size.width as usize * size.height as usize * 4;
            }
        } else {
            cache.stats.tile_hits += 1;
        }
        if !cache.tiles.is_empty() {
            let count = canvas.save();
            canvas.set_matrix(&sk::M44::from(matrix([1., 0., 0., 1., 0., 0.])));
            let mut p = Paint::default();
            p.set_blend_mode(sk::BlendMode::SrcOver);
            for (image, r) in &cache.tiles {
                canvas.draw_image(image, (r.left, r.top), Some(&p));
            }
            canvas.restore_to_count(count);
            if cache.stats.tile_bytes > 16 * 1024 * 1024 {
                cache.tiles.clear();
                cache.stats.tile_bytes = 0;
            }
            return;
        }
    }
    canvas.draw_picture(&picture, None, None);
}
pub(crate) fn validate_buffer(bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > 64 * 1024 * 1024 {
        Err(Error::Buffer(vellum_types::display::BufferError::TooLarge))
    } else {
        Ok(())
    }
}
