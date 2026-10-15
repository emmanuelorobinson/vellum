// TEST DATA: synthetic display-list fixtures, independent of the host's fonts or GPU.
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
fn pixels_and_real_export_formats() {
    let renderer = Renderer::new().unwrap();
    let scene = renderer.prepare(&fixture(), &Resources::default()).unwrap();
    let page = Page {
        width: 40,
        height: 40,
        scene: &scene,
    };
    let rgba = renderer.rgba(&page).unwrap();
    assert_eq!(
        &rgba[(12 * 40 + 12) * 4..(12 * 40 + 12) * 4 + 4],
        &[255, 0, 0, 255]
    );
    assert_eq!(&rgba[0..4], &[0, 0, 0, 0]);
    assert!(
        renderer
            .png(&page)
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    let pdf = renderer.pdf(&[page, page]).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(String::from_utf8_lossy(&pdf).contains("/Count 2"));
    let svg = renderer.svg(&page).unwrap();
    assert!(svg.contains("<svg") && svg.contains("<path") && svg.contains("#ff0000"));
}
#[test]
fn validates_before_drawing_and_refuses_missing_resources() {
    let renderer = Renderer::new().unwrap();
    let mut list = fixture();
    list.commands.push(DrawCmd::Restore);
    assert!(matches!(
        renderer.prepare(&list, &Resources::default()),
        Err(Error::Buffer(_))
    ));
    let mut list = fixture();
    list.resources[1] = Resource::Paint(RenderPaint::Image(RenderImage {
        asset: AssetId::new("missing").unwrap(),
        fit: ImageFit::Fill,
        crop: None,
        opacity: 1.,
    }));
    assert!(matches!(
        renderer.prepare(&list, &Resources::default()),
        Err(Error::MissingImage(_))
    ));
}

fn render(list: &DisplayList, resources: &Resources) -> Vec<u8> {
    let r = Renderer::new().unwrap();
    let scene = r.prepare(list, resources).unwrap();
    r.rgba(&Page {
        width: 80,
        height: 80,
        scene: &scene,
    })
    .unwrap()
}
fn pixel(rgba: &[u8], x: usize, y: usize) -> &[u8] {
    &rgba[(y * 80 + x) * 4..(y * 80 + x) * 4 + 4]
}
#[test]
fn clips_survive_absolute_transform_and_opacity_is_a_group() {
    let mut list = fixture();
    list.commands = vec![
        DrawCmd::Save,
        DrawCmd::Transform([1., 0., 0., 1., 10., 10.]),
        DrawCmd::Clip { shape: 0 },
        DrawCmd::Layer {
            opacity: 0.5,
            blend: BlendMode::Normal,
        },
        DrawCmd::Transform([1., 0., 0., 1., 20., 20.]),
        DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        },
        DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        },
        DrawCmd::Restore,
        DrawCmd::Restore,
    ];
    let rgba = render(&list, &Resources::default());
    assert!((127..=128).contains(&pixel(&rgba, 25, 25)[3]));
    assert_eq!(pixel(&rgba, 35, 25)[3], 0);
    assert_eq!(pixel(&rgba, 15, 15)[3], 0);
}
#[test]
fn per_side_widths_and_all_alignments() {
    for (align, inside, outside) in [
        (StrokeAlign::Inside, 255, 0),
        (StrokeAlign::Outside, 0, 255),
        (StrokeAlign::Center, 255, 255),
    ] {
        let mut list = fixture();
        list.resources[0] = Resource::Shape(Shape::Rect {
            size: [40., 40.],
            radii: [0.; 4],
        });
        list.resources.push(Resource::Stroke(RenderStroke {
            widths: [8., 6., 4., 2.],
            align,
            cap: StrokeCap::Butt,
            join: StrokeJoin::Miter,
            dash: vec![],
        }));
        list.commands = vec![
            DrawCmd::Transform([1., 0., 0., 1., 20., 20.]),
            DrawCmd::Path {
                shape: 0,
                paint: 1,
                stroke: Some(2),
            },
        ];
        let rgba = render(&list, &Resources::default());
        assert_eq!(pixel(&rgba, 40, 21)[3], inside);
        assert_eq!(pixel(&rgba, 40, 18)[3], outside);
        assert_eq!(pixel(&rgba, 25, 40)[3], 0); // Left is only 2 px wide.
        assert_eq!(pixel(&rgba, 40, 40)[3], 0);
    }
}
#[test]
fn line_caps_dashes_and_zero_width() {
    let mut list = fixture();
    list.resources[0] = Resource::Shape(Shape::Line { width: 40. });
    for cap in StrokeCap::ALL {
        list.resources.truncate(2);
        list.resources.push(Resource::Stroke(RenderStroke {
            widths: [8.; 4],
            align: StrokeAlign::Center,
            cap,
            join: StrokeJoin::Round,
            dash: vec![],
        }));
        list.commands = vec![
            DrawCmd::Transform([1., 0., 0., 1., 20., 20.]),
            DrawCmd::Path {
                shape: 0,
                paint: 1,
                stroke: Some(2),
            },
        ];
        let rgba = render(&list, &Resources::default());
        assert_eq!(pixel(&rgba, 18, 20)[3] > 0, cap != StrokeCap::Butt);
    }
    let Resource::Stroke(s) = &mut list.resources[2] else {
        unreachable!()
    };
    s.cap = StrokeCap::Butt;
    s.dash = vec![8., 8.];
    let rgba = render(&list, &Resources::default());
    assert_eq!(pixel(&rgba, 24, 20)[3], 255);
    assert_eq!(pixel(&rgba, 32, 20)[3], 0);
    let Resource::Stroke(s) = &mut list.resources[2] else {
        unreachable!()
    };
    s.widths = [0.; 4];
    assert!(render(&list, &Resources::default()).iter().all(|n| *n == 0));
}
#[test]
fn images_fit_fill_stretch_crop_and_missing_data() {
    let r = Renderer::new().unwrap();
    let mut source = fixture();
    source.resources[0] = Resource::Shape(Shape::Rect {
        size: [20., 10.],
        radii: [0.; 4],
    });
    source.commands = vec![DrawCmd::Path {
        shape: 0,
        paint: 1,
        stroke: None,
    }];
    let s = r.prepare(&source, &Resources::default()).unwrap();
    let png = r
        .png(&Page {
            width: 20,
            height: 10,
            scene: &s,
        })
        .unwrap();
    let id = AssetId::new("image").unwrap();
    let resources = Resources {
        images: [(id.clone(), png.into())].into(),
        ..Default::default()
    };
    for (fit, top) in [
        (ImageFit::Fit, 0),
        (ImageFit::Fill, 255),
        (ImageFit::Stretch, 255),
    ] {
        let mut list = DisplayList {
            resources: vec![Resource::Image(RenderImage {
                asset: id.clone(),
                fit,
                crop: None,
                opacity: 1.,
            })],
            commands: vec![DrawCmd::Image {
                image: 0,
                dst: [20., 20., 40., 40.],
            }],
        };
        let rgba = render(&list, &resources);
        assert_eq!(pixel(&rgba, 40, 22)[3], top);
        assert_eq!(pixel(&rgba, 40, 40), &[255, 0, 0, 255]);
        assert_eq!(pixel(&rgba, 10, 40)[3], 0);
        let Resource::Image(image) = &mut list.resources[0] else {
            unreachable!()
        };
        image.crop = Some([0.5, 0., 0., 1., 0.5, 0.]);
        let rgba = render(&list, &resources);
        assert_eq!(pixel(&rgba, 22, 40)[3], 0);
        assert_eq!(pixel(&rgba, 58, 40)[3], 255);
        let scene = r.prepare(&list, &resources).unwrap();
        let svg = r
            .svg(&Page {
                width: 80,
                height: 80,
                scene: &scene,
            })
            .unwrap();
        assert!(svg.contains("data:image/png;base64,") && !svg.contains("href=\"http"));
    }
}
#[test]
fn vector_arcs_boolean_and_fill_rules_have_coverage() {
    let shapes = [
        Shape::Ellipse {
            size: [40., 40.],
            inner: 0.5,
            start: 0.,
            sweep: 360.,
        },
        Shape::Polygon {
            size: [40., 40.],
            sides: 5,
            radius: 3.,
        },
        Shape::Star {
            size: [40., 40.],
            points: 5,
            inner: 0.5,
        },
        Shape::Vector {
            data: "M0 0 H40 V40 H0 Z M10 10 H30 V30 H10 Z".into(),
            view_box: [0., 0., 40., 40.],
            size: [40., 40.],
            fill_rule: FillRule::Evenodd,
        },
        Shape::Boolean {
            op: BoolOp::Subtract,
            operands: vec![
                ShapeOperand {
                    transform: [1., 0., 0., 1., 0., 0.],
                    shape: Shape::Rect {
                        size: [40., 40.],
                        radii: [0.; 4],
                    },
                },
                ShapeOperand {
                    transform: [1., 0., 0., 1., 10., 10.],
                    shape: Shape::Rect {
                        size: [20., 20.],
                        radii: [0.; 4],
                    },
                },
            ],
        },
    ];
    for (i, s) in shapes.into_iter().enumerate() {
        let mut list = fixture();
        list.resources[0] = Resource::Shape(s);
        let rgba = render(&list, &Resources::default());
        assert!(
            rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0),
            "shape {i}"
        );
        if [0, 3, 4].contains(&i) {
            assert_eq!(pixel(&rgba, 28, 28)[3], 0);
        }
    }
}
#[test]
fn nonfinite_resource_scalars_are_refused() {
    let r = Renderer::new().unwrap();
    let mut list = fixture();
    list.resources.push(Resource::Stroke(RenderStroke {
        widths: [f32::NAN; 4],
        align: StrokeAlign::Center,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
        dash: vec![],
    }));
    list.commands = vec![DrawCmd::Path {
        shape: 0,
        paint: 1,
        stroke: Some(2),
    }];
    assert!(r.prepare(&list, &Resources::default()).is_err());
}
#[test]
fn typed_glyphs_cannot_bypass_wire_number_validation() {
    let mut list = fixture();
    list.resources.push(Resource::Glyphs(RenderGlyphRun {
        font: 0,
        size: 20.,
        glyphs: vec![RenderGlyph {
            id: 0,
            cluster: 0,
            position: [f32::NAN, 0.],
            advance: 0.,
        }],
        normalized_coords: vec![],
        embolden: false,
        skew: None,
        underline: None,
        strikethrough: None,
        link: None,
    }));
    list.commands = vec![DrawCmd::GlyphRun {
        glyphs: 2,
        paint: 1,
        stroke: None,
    }];
    assert!(matches!(
        Renderer::new()
            .unwrap()
            .prepare(&list, &Resources::default()),
        Err(Error::Buffer(_))
    ));
}
