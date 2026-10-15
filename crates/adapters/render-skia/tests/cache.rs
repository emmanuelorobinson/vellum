// TEST DATA: cache equivalence and invalidation, deterministic insertion order (no wall clock).
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::display::*;
fn list(color: [u8; 4]) -> DisplayList {
    DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [50., 50.],
                radii: [0.; 4],
            }),
            Resource::Paint(RenderPaint::Solid(color)),
        ],
        commands: vec![DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        }],
    }
}
#[test]
fn unchanged_frames_reuse_pictures_and_zoomed_out_tiles_without_stale_pixels() {
    let r = Renderer::new().unwrap();
    let resources = Resources::default();
    let bytes = list([255, 0, 0, 255]).to_bytes().unwrap();
    let scene = r.prepare_frame("frame-a", &bytes, &resources).unwrap();
    let a = r
        .rgba(&Page {
            width: 80,
            height: 80,
            scene: &scene,
        })
        .unwrap();
    let same = r.prepare_frame("frame-a", &bytes, &resources).unwrap();
    assert_eq!(
        a,
        r.rgba(&Page {
            width: 80,
            height: 80,
            scene: &same
        })
        .unwrap()
    );
    assert_eq!(same.cache_stats().pictures, 1);
    assert!(same.cache_stats().picture_hits > 0);
    let mut surface = skia_safe::surfaces::raster_n32_premul((80, 80)).unwrap();
    same.draw_cached(surface.canvas(), 0.25);
    same.draw_cached(surface.canvas(), 0.25);
    assert_eq!(same.cache_stats().tile_builds, 1);
    assert_eq!(same.cache_stats().tile_hits, 1);
    let changed = r
        .prepare_frame(
            "frame-a",
            &list([0, 255, 0, 255]).to_bytes().unwrap(),
            &resources,
        )
        .unwrap();
    assert_ne!(
        a,
        r.rgba(&Page {
            width: 80,
            height: 80,
            scene: &changed
        })
        .unwrap()
    );
    assert_eq!(changed.cache_stats().pictures, 1);
    let bad = r.prepare_frame("frame-a", b"invalid", &resources);
    assert!(bad.is_err());
    assert_eq!(
        a,
        r.rgba(&Page {
            width: 80,
            height: 80,
            scene: &same
        })
        .unwrap()
    );
    same.draw_cached(
        skia_safe::surfaces::raster_n32_premul((100, 80))
            .unwrap()
            .canvas(),
        0.25,
    );
    assert_eq!(same.cache_stats().tile_builds, 2);
}
#[test]
fn exact_resource_changes_and_bounded_frame_eviction_invalidate_pictures() {
    use vellum_types::AssetId;
    let renderer = Renderer::new().unwrap();
    let bytes = list([255, 0, 0, 255]).to_bytes().unwrap();
    let mut resources = Resources::default();
    let first = renderer.prepare_frame("first", &bytes, &resources).unwrap();
    renderer
        .rgba(&Page {
            width: 80,
            height: 80,
            scene: &first,
        })
        .unwrap();
    resources
        .images
        .insert(AssetId::new("unused").unwrap(), vec![1].into());
    let changed = renderer.prepare_frame("first", &bytes, &resources).unwrap();
    assert_eq!(changed.cache_stats().pictures, 0);
    renderer
        .rgba(&Page {
            width: 80,
            height: 80,
            scene: &changed,
        })
        .unwrap();
    assert_eq!(
        renderer
            .prepare_frame("first", &bytes, &resources)
            .unwrap()
            .cache_stats()
            .pictures,
        1
    );
    for i in 0..9 {
        renderer
            .prepare_frame(&format!("f{i}"), &bytes, &resources)
            .unwrap();
    }
    assert_eq!(
        renderer
            .prepare_frame("first", &bytes, &resources)
            .unwrap()
            .cache_stats()
            .pictures,
        0
    );
    let mut surface = skia_safe::surfaces::raster_n32_premul((80, 80)).unwrap();
    first.draw_cached(surface.canvas(), 0.25);
    let mut pixels = vec![0; 80 * 80 * 4];
    let info = skia_safe::ImageInfo::new(
        (80, 80),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    assert!(surface.read_pixels(&info, &mut pixels, 80 * 4, (0, 0)));
    assert_eq!(
        pixels,
        renderer
            .rgba(&Page {
                width: 80,
                height: 80,
                scene: &first
            })
            .unwrap()
    );
}
