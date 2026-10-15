// TEST DATA: the native composition root combines text and renderer adapters.
use vellum_render_skia::{Error, Page, Renderer, Resources};
use vellum_types::{display::*, *};
fn fixture() -> DisplayList {
    DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [20., 20.],
                radii: [0.; 4],
            }),
            Resource::Paint(RenderPaint::Solid([255, 0, 0, 255])),
        ],
        commands: vec![
            DrawCmd::Save,
            DrawCmd::Transform([1., 0., 0., 1., 8., 8.]),
            DrawCmd::Path {
                shape: 0,
                paint: 1,
                stroke: None,
            },
            DrawCmd::Restore,
        ],
    }
}
#[test]
fn shaped_text_and_colour_emoji_use_only_supplied_bytes() {
    use vellum_core::ports::{FontSource, RichText, TextEngine};
    let fonts = vellum_text_parley::ParleyFonts::new();
    let text = vellum_text_parley::ParleyText::new(fonts.clone());
    let mut rich = RichText::plain("Hi 😀", "Inter", 26.);
    rich.runs[0].underline = true;
    let layout = text.layout(&rich, 80.);
    assert!(layout.problems.is_empty(), "{:?}", layout.problems);
    let mut list = fixture();
    list.commands.clear();
    let mut resources = Resources::default();
    for line in layout.lines {
        for run in line.glyph_runs {
            resources
                .fonts
                .insert(run.font, fonts.data(run.font).unwrap());
            let index = list.resources.len() as u32;
            list.resources.push(Resource::Glyphs(RenderGlyphRun {
                font: run.font.0,
                size: run.size as f32,
                normalized_coords: run.normalized_coords,
                embolden: run.embolden,
                skew: run.skew.map(|v| v as f32),
                underline: run.underline.map(|d| [d.offset as f32, d.thickness as f32]),
                strikethrough: None,
                link: None,
                glyphs: run
                    .glyphs
                    .iter()
                    .map(|g| RenderGlyph {
                        id: g.id,
                        cluster: g.cluster as u32,
                        position: [(line.x + g.x) as f32, (line.y + line.baseline + g.y) as f32],
                        advance: g.advance as f32,
                    })
                    .collect(),
            }));
            list.commands.push(DrawCmd::GlyphRun {
                glyphs: index,
                paint: 1,
                stroke: None,
            });
        }
    }
    let r = Renderer::new().unwrap();
    let scene = r.prepare(&list, &resources).unwrap();
    let page = Page {
        width: 80,
        height: 80,
        scene: &scene,
    };
    let rgba = r.rgba(&page).unwrap();
    assert!(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 100 && p[1] > 80 && p[2] < 60 && p[3] > 100),
        "emoji retains yellow pixels"
    );
    let svg = r.svg(&page).unwrap();
    assert!(svg.contains("<path") && svg.contains("data:image/png;base64,"));
    assert!(!svg.contains("<text"));
    assert!(matches!(
        r.prepare(&list, &Resources::default()),
        Err(Error::MissingFont(_))
    ));
}

#[test]
fn aligned_text_stroke_is_geometry_on_all_backends() {
    use vellum_core::ports::FontSource;
    let fonts = vellum_text_parley::ParleyFonts::new();
    let handle = fonts.resolve("Inter", 400, FontStyle::Normal).unwrap();
    let data = fonts.data(handle).unwrap();
    // Inter's glyph 1 has an outline; rendering must not require a platform text-layout API.
    let mut list = fixture();
    list.resources.push(Resource::Glyphs(RenderGlyphRun {
        font: handle.0,
        size: 40.,
        glyphs: vec![RenderGlyph {
            id: 1,
            cluster: 0,
            position: [20., 50.],
            advance: 30.,
        }],
        normalized_coords: vec![],
        embolden: false,
        skew: None,
        underline: None,
        strikethrough: None,
        link: None,
    }));
    list.resources.push(Resource::Stroke(RenderStroke {
        widths: [2.; 4],
        align: StrokeAlign::Inside,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
        dash: vec![],
    }));
    list.commands = vec![DrawCmd::GlyphRun {
        glyphs: 2,
        paint: 1,
        stroke: Some(3),
    }];
    let resources = Resources {
        fonts: [(handle, data)].into(),
        ..Default::default()
    };
    let r = Renderer::new().unwrap();
    let scene = r.prepare(&list, &resources).unwrap();
    assert!(
        r.svg(&Page {
            width: 80,
            height: 80,
            scene: &scene
        })
        .unwrap()
        .contains("<path")
    );
}
