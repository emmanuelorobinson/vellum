// TEST DATA: analytic renderer checks; no GPU or system font dependency.
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::{display::*, *};
fn list(paint: RenderPaint, effects: Vec<RenderEffect>, blend: BlendMode) -> DisplayList {
    let mut list = DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [40., 40.],
                radii: [0.; 4],
            }),
            Resource::Paint(paint),
        ],
        commands: vec![
            DrawCmd::Transform([1., 0., 0., 1., 20., 20.]),
            DrawCmd::Layer { opacity: 1., blend },
        ],
    };
    for effect in effects {
        let i = list.resources.len() as u32;
        list.resources.push(Resource::Effect(effect));
        list.commands.push(DrawCmd::Effect { effect: i });
    }
    list.commands.extend([
        DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        },
        DrawCmd::Restore,
    ]);
    list
}
fn pixels(list: &DisplayList) -> Vec<u8> {
    let r = Renderer::new().unwrap();
    let scene = r.prepare(list, &Resources::default()).unwrap();
    r.rgba(&Page {
        width: 80,
        height: 80,
        scene: &scene,
    })
    .unwrap()
}
fn pixel(p: &[u8], x: usize, y: usize) -> &[u8] {
    &p[(y * 80 + x) * 4..(y * 80 + x) * 4 + 4]
}
fn solid() -> RenderPaint {
    RenderPaint::Solid([255, 0, 0, 255])
}
fn shadow() -> RenderShadow {
    RenderShadow {
        offset: [8., 0.],
        blur: 0.,
        spread: 0.,
        color: [0, 0, 255, 255],
        blend: BlendMode::Normal,
    }
}
#[test]
fn gradients_map_unit_coordinates_and_preserve_transparency() {
    for kind in GradientKind::ALL {
        let p = pixels(&list(
            RenderPaint::Gradient {
                kind,
                transform: [1., 0., 0., 1., 0., 0.],
                stops: vec![
                    RenderStop {
                        position: 0.,
                        color: [255, 0, 0, 255],
                    },
                    RenderStop {
                        position: 1.,
                        color: [0, 0, 255, 255],
                    },
                ],
            },
            vec![],
            BlendMode::Normal,
        ));
        assert_ne!(pixel(&p, 25, 40), pixel(&p, 55, 40), "{kind:?}");
        assert_eq!(pixel(&p, 10, 40)[3], 0);
        if kind == GradientKind::Linear {
            assert!(pixel(&p, 21, 40)[0] > 240);
            assert!(pixel(&p, 58, 40)[2] > 240);
        }
    }
}
#[test]
fn mesh_interpolates_four_corners_and_honors_handles() {
    let points = vec![
        ([0., 0.], [255, 0, 0, 255]),
        ([1., 0.], [0, 255, 0, 255]),
        ([0., 1.], [0, 0, 255, 255]),
        ([1., 1.], [255, 255, 255, 255]),
    ]
    .into_iter()
    .map(|(position, color)| RenderMeshPoint {
        position,
        color,
        handles: None,
    })
    .collect();
    let mut l = list(
        RenderPaint::Mesh {
            columns: 2,
            rows: 2,
            points,
        },
        vec![],
        BlendMode::Normal,
    );
    let p = pixels(&l);
    assert!(pixel(&p, 21, 21)[0] > 220);
    assert!(pixel(&p, 58, 21)[1] > 220);
    assert!(pixel(&p, 21, 58)[2] > 220);
    let Resource::Paint(RenderPaint::Mesh { points, .. }) = &mut l.resources[1] else {
        unreachable!()
    };
    points[0].handles = Some([[0., 0.], [0.33, 0.5], [0., 0.], [0., 0.33]]);
    assert_ne!(p, pixels(&l));
}
#[test]
fn shadows_blur_and_negative_spread_have_pixel_semantics() {
    let l = list(
        solid(),
        vec![RenderEffect::DropShadow(shadow())],
        BlendMode::Normal,
    );
    let p = pixels(&l);
    assert_eq!(pixel(&p, 64, 40), [0, 0, 255, 255]);
    assert_eq!(pixel(&p, 55, 40), [255, 0, 0, 255]);
    let p = pixels(&list(
        solid(),
        vec![RenderEffect::InnerShadow(shadow())],
        BlendMode::Normal,
    ));
    assert_eq!(pixel(&p, 22, 40), [0, 0, 255, 255]);
    assert_eq!(pixel(&p, 40, 40), [255, 0, 0, 255]);
    assert_eq!(pixel(&p, 64, 40)[3], 0);
    let p = pixels(&list(
        solid(),
        vec![RenderEffect::LayerBlur(6.)],
        BlendMode::Normal,
    ));
    assert!(pixel(&p, 18, 40)[3] > 0);
    assert!(pixel(&p, 21, 40)[3] < 255);
    let mut s = shadow();
    s.spread = -4.;
    let p = pixels(&list(
        solid(),
        vec![RenderEffect::DropShadow(s)],
        BlendMode::Normal,
    ));
    assert_eq!(pixel(&p, 66, 40)[3], 0);
}
#[test]
fn all_blends_work_and_multiply_is_analytic() {
    for blend in BlendMode::ALL {
        let mut l = list(RenderPaint::Solid([128, 64, 200, 255]), vec![], blend);
        l.resources
            .push(Resource::Paint(RenderPaint::Solid([100, 200, 150, 255])));
        l.commands.splice(
            0..0,
            [
                DrawCmd::Transform([1., 0., 0., 1., 20., 20.]),
                DrawCmd::Path {
                    shape: 0,
                    paint: 2,
                    stroke: None,
                },
            ],
        );
        let p = pixels(&l);
        assert_eq!(pixel(&p, 40, 40)[3], 255);
        if blend == BlendMode::Multiply {
            for (a, b) in pixel(&p, 40, 40)[..3].iter().zip([50, 50, 118]) {
                assert!((*a as i32 - b).abs() <= 1);
            }
        }
        if blend == BlendMode::LinearBurn {
            assert_eq!(pixel(&p, 40, 40), [0, 9, 95, 255]);
        }
    }
}
#[test]
fn invalid_appearance_and_shaders_are_explicit_errors() {
    let scene = Renderer::new()
        .unwrap()
        .prepare(
            &list(
                RenderPaint::Shader {
                    src: "untrusted".into(),
                    uniforms: Default::default(),
                },
                vec![],
                BlendMode::Normal,
            ),
            &Resources::default(),
        )
        .unwrap();
    assert_eq!(scene.shader_errors().len(), 1);
    assert_eq!(scene.shader_errors()[0].src, "untrusted");

    for paint in [
        RenderPaint::Gradient {
            kind: GradientKind::Linear,
            transform: [1., 0., 0., 1., 0., 0.],
            stops: vec![],
        },
        RenderPaint::Mesh {
            columns: 2,
            rows: 2,
            points: vec![],
        },
    ] {
        assert!(
            Renderer::new()
                .unwrap()
                .prepare(
                    &list(paint, vec![], BlendMode::Normal),
                    &Resources::default()
                )
                .is_err()
        );
    }
    assert!(
        Renderer::new()
            .unwrap()
            .prepare(
                &list(
                    solid(),
                    vec![RenderEffect::LayerBlur(-1.)],
                    BlendMode::Normal
                ),
                &Resources::default()
            )
            .is_err()
    );
}
#[test]
fn background_blur_is_masked_to_layer_geometry() {
    let mut l = list(
        RenderPaint::Solid([255, 255, 255, 0]),
        vec![RenderEffect::BackgroundBlur(10.)],
        BlendMode::Normal,
    );
    l.resources.push(Resource::Shape(Shape::Rect {
        size: [40., 80.],
        radii: [0.; 4],
    }));
    l.resources
        .push(Resource::Paint(RenderPaint::Solid([255, 0, 0, 255])));
    l.resources
        .push(Resource::Paint(RenderPaint::Solid([0, 0, 255, 255])));
    l.commands.splice(
        0..0,
        [
            DrawCmd::Path {
                shape: 3,
                paint: 4,
                stroke: None,
            },
            DrawCmd::Transform([1., 0., 0., 1., 40., 0.]),
            DrawCmd::Path {
                shape: 3,
                paint: 5,
                stroke: None,
            },
        ],
    );
    let p = pixels(&l);
    assert_eq!(pixel(&p, 39, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(&p, 40, 10), [0, 0, 255, 255]);
    assert!(pixel(&p, 39, 40)[2] > 50);
    assert!(pixel(&p, 40, 40)[0] > 50);
}
#[test]
fn basic_plan_omits_effects_but_keeps_geometry_and_reports_simplification() {
    let r = Renderer::new().unwrap();
    let l = list(
        solid(),
        vec![RenderEffect::DropShadow(shadow())],
        BlendMode::Normal,
    );
    let plan = r
        .basic_plan(&l.to_bytes().unwrap(), &Resources::default())
        .unwrap();
    let plan: String = String::from_utf8(plan).unwrap();
    assert!(plan.contains("\"op\":\"path\""));
    assert!(plan.contains("effects-omitted"));
    assert!(!plan.contains("DropShadow"));
}
#[test]
fn gradient_strokes_on_a_line_have_coverage() {
    let mut l = list(
        RenderPaint::Gradient {
            kind: GradientKind::Linear,
            transform: [1., 0., 0., 1., 0., 0.],
            stops: vec![
                RenderStop {
                    position: 0.,
                    color: [255, 0, 0, 255],
                },
                RenderStop {
                    position: 1.,
                    color: [0, 0, 255, 255],
                },
            ],
        },
        vec![],
        BlendMode::Normal,
    );
    l.resources[0] = Resource::Shape(Shape::Line { width: 40. });
    l.resources.push(Resource::Stroke(RenderStroke {
        widths: [8.; 4],
        align: StrokeAlign::Center,
        cap: StrokeCap::Butt,
        join: StrokeJoin::Miter,
        dash: vec![],
    }));
    for cmd in &mut l.commands {
        if let DrawCmd::Path { stroke, .. } = cmd {
            *stroke = Some(2);
        }
    }
    let p = pixels(&l);
    assert!(pixel(&p, 22, 20)[0] > 220);
    assert!(pixel(&p, 57, 20)[2] > 220);
}
#[test]
fn advanced_exports_preserve_runtime_blends_and_filters() {
    let r = Renderer::new().unwrap();
    for l in [
        list(solid(), vec![], BlendMode::LinearBurn),
        list(
            solid(),
            vec![
                RenderEffect::BackgroundBlur(8.),
                RenderEffect::DropShadow(shadow()),
            ],
            BlendMode::Normal,
        ),
    ] {
        let scene = r.prepare(&l, &Resources::default()).unwrap();
        let page = Page {
            width: 80,
            height: 80,
            scene: &scene,
        };
        assert!(r.svg(&page).unwrap().contains("data:image/png;base64,"));
        let pdf = r.pdf(&[page, page]).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(String::from_utf8_lossy(&pdf).contains("/Count 2"));
    }
}
