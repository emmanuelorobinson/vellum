//! TEST DATA: native/browser renderer parity inputs and captures. No host font discovery.
use std::{fs, path::Path};
use vellum_core::ports::{FontSource, RichText, TextEngine};
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::{display::*, *};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).expect("output directory");
    let out = Path::new(&out);
    fs::create_dir_all(out)?;
    let renderer = Renderer::new()?;
    let fonts = vellum_text_parley::ParleyFonts::new();
    let text = vellum_text_parley::ParleyText::new(fonts.clone());
    let mut resources = Resources::default();
    let mut list = DisplayList::default();
    list.resources
        .push(Resource::Paint(RenderPaint::Solid([48, 90, 210, 255])));
    list.resources
        .push(Resource::Paint(RenderPaint::Solid([230, 130, 20, 255])));
    let shapes = [
        Shape::Rect {
            size: [90., 40.],
            radii: [8., 4., 12., 0.],
        },
        Shape::Ellipse {
            size: [60., 50.],
            inner: 0.5,
            start: 20.,
            sweep: 280.,
        },
        Shape::Polygon {
            size: [60., 50.],
            sides: 5,
            radius: 4.,
        },
        Shape::Star {
            size: [60., 50.],
            points: 5,
            inner: 0.45,
        },
        Shape::Vector {
            data: "M0 0 C60 0 0 60 60 60 L0 60 Z".into(),
            view_box: [0., 0., 60., 60.],
            size: [60., 50.],
            fill_rule: FillRule::Nonzero,
        },
    ];
    let stroke = list.resources.len() as u32;
    list.resources.push(Resource::Stroke(RenderStroke {
        widths: [6., 4., 2., 8.],
        align: StrokeAlign::Center,
        cap: StrokeCap::Round,
        join: StrokeJoin::Bevel,
        dash: vec![10., 4.],
    }));
    for (i, shape) in shapes.into_iter().enumerate() {
        let index = list.resources.len() as u32;
        list.resources.push(Resource::Shape(shape));
        list.commands.extend([
            DrawCmd::Save,
            DrawCmd::Transform([
                1.,
                0.,
                0.,
                1.,
                12. + i as f32 % 3. * 80.,
                12. + (i / 3) as f32 * 65.,
            ]),
            DrawCmd::Path {
                shape: index,
                paint: 0,
                stroke: None,
            },
        ]);
        if i == 0 {
            list.commands.push(DrawCmd::Path {
                shape: index,
                paint: 1,
                stroke: Some(stroke),
            });
        }
        list.commands.push(DrawCmd::Restore);
    }
    let mut rich = RichText::plain("Variable Inter\n😀 👩🏽‍💻", "Inter", 24.);
    rich.runs[0].weight = 700;
    rich.runs[0].underline = true;
    let layout = text.layout(&rich, 250.);
    assert!(layout.problems.is_empty());
    for line in layout.lines {
        for run in line.glyph_runs {
            resources
                .fonts
                .insert(run.font, fonts.data(run.font).unwrap());
            let id = list.resources.len() as u32;
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
            list.commands.extend([
                DrawCmd::Save,
                DrawCmd::Transform([1., 0., 0., 1., 10., 145.]),
                DrawCmd::GlyphRun {
                    glyphs: id,
                    paint: 1,
                    stroke: None,
                },
                DrawCmd::Restore,
            ]);
        }
    }
    let image_list = DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [20., 10.],
                radii: [0.; 4],
            }),
            Resource::Paint(RenderPaint::Solid([20, 160, 90, 255])),
        ],
        commands: vec![DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        }],
    };
    let image_scene = renderer.prepare(&image_list, &Resources::default())?;
    let image = renderer.png(&Page {
        width: 20,
        height: 10,
        scene: &image_scene,
    })?;
    let asset = AssetId::new("renderer-image").unwrap();
    fs::write(out.join("image.png"), &image)?;
    resources.images.insert(asset.clone(), image.into());
    let id = list.resources.len() as u32;
    list.resources.push(Resource::Image(RenderImage {
        asset,
        fit: ImageFit::Fit,
        crop: Some([0.8, 0., 0., 1., 0.1, 0.]),
        opacity: 0.8,
    }));
    list.commands.extend([
        DrawCmd::Save,
        DrawCmd::Transform([1., 0., 0., 1., 180., 85.]),
        DrawCmd::Image {
            image: id,
            dst: [0., 0., 60., 50.],
        },
        DrawCmd::Restore,
    ]);
    let scene = renderer.prepare(&list, &resources)?;
    let page = Page {
        width: 256,
        height: 256,
        scene: &scene,
    };
    fs::write(out.join("scene.bin"), list.to_bytes()?)?;
    fs::write(out.join("native.rgba"), renderer.rgba(&page)?)?;
    fs::write(out.join("native.png"), renderer.png(&page)?)?;
    fs::write(out.join("native.svg"), renderer.svg(&page)?)?;
    fs::write(out.join("native.pdf"), renderer.pdf(&[page, page])?)?;
    for (handle, data) in resources.fonts {
        fs::write(out.join(format!("font-{}.ttf", handle.0)), data.bytes)?;
    }
    Ok(())
}
