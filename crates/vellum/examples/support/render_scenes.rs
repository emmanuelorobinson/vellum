// TEST DATA: original analytic fixtures, shared by recording, parity, and performance harnesses.
use vellum_types::{display::*, *};
pub fn scenes() -> Vec<(String, DisplayList)> {
    let mut scenes = vec![];
    let base = || DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [128., 128.],
                radii: [12., 8., 20., 0.],
            }),
            Resource::Paint(RenderPaint::Solid([210, 50, 80, 255])),
        ],
        commands: vec![
            DrawCmd::Save,
            DrawCmd::Transform([1., 0., 0., 1., 64., 64.]),
            DrawCmd::Path {
                shape: 0,
                paint: 1,
                stroke: None,
            },
            DrawCmd::Restore,
        ],
    };
    scenes.push(("solid".into(), base()));
    for kind in GradientKind::ALL {
        let mut l = base();
        l.resources[1] = Resource::Paint(RenderPaint::Gradient {
            kind,
            transform: [0.8, 0.2, -0.1, 0.9, 0.1, 0.],
            stops: vec![
                RenderStop {
                    position: 0.,
                    color: [240, 40, 20, 255],
                },
                RenderStop {
                    position: 0.5,
                    color: [20, 190, 100, 128],
                },
                RenderStop {
                    position: 1.,
                    color: [30, 60, 220, 255],
                },
            ],
        });
        scenes.push((format!("gradient-{}", kind.as_str()), l));
    }
    for handled in [false, true] {
        let mut l = base();
        l.resources[1] = Resource::Paint(RenderPaint::Mesh {
            columns: 2,
            rows: 2,
            points: [
                ([0., 0.], [230, 30, 20, 255]),
                ([1., 0.], [20, 230, 30, 255]),
                ([0., 1.], [30, 20, 230, 255]),
                ([1., 1.], [230, 230, 230, 128]),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (position, color))| RenderMeshPoint {
                position,
                color,
                handles: if handled && i == 0 {
                    Some([[0., 0.], [0.33, 0.25], [0., 0.], [0.2, 0.33]])
                } else {
                    None
                },
            })
            .collect(),
        });
        scenes.push((if handled { "mesh-handles" } else { "mesh" }.into(), l));
    }
    for blend in BlendMode::ALL {
        let mut l = base();
        l.resources
            .push(Resource::Paint(RenderPaint::Solid([50, 170, 210, 255])));
        l.commands = vec![
            DrawCmd::Transform([1., 0., 0., 1., 32., 32.]),
            DrawCmd::Path {
                shape: 0,
                paint: 2,
                stroke: None,
            },
            DrawCmd::Layer {
                opacity: 0.75,
                blend,
            },
            DrawCmd::Transform([1., 0., 0., 1., 96., 96.]),
            DrawCmd::Path {
                shape: 0,
                paint: 1,
                stroke: None,
            },
            DrawCmd::Restore,
        ];
        scenes.push((format!("blend-{}", blend.as_str()), l));
    }
    for (name, effect) in [
        (
            "drop-shadow",
            RenderEffect::DropShadow(RenderShadow {
                offset: [12., 8.],
                blur: 12.,
                spread: 4.,
                color: [10, 20, 80, 200],
                blend: BlendMode::Multiply,
            }),
        ),
        (
            "negative-spread",
            RenderEffect::DropShadow(RenderShadow {
                offset: [12., 8.],
                blur: 8.,
                spread: -8.,
                color: [20, 60, 10, 255],
                blend: BlendMode::Normal,
            }),
        ),
        (
            "inner-shadow",
            RenderEffect::InnerShadow(RenderShadow {
                offset: [8., 4.],
                blur: 12.,
                spread: 4.,
                color: [20, 20, 80, 220],
                blend: BlendMode::Multiply,
            }),
        ),
        ("layer-blur", RenderEffect::LayerBlur(12.)),
        ("background-blur", RenderEffect::BackgroundBlur(18.)),
    ] {
        let mut l = base();
        l.resources.push(Resource::Effect(effect));
        l.commands.insert(
            2,
            DrawCmd::Layer {
                opacity: 0.8,
                blend: BlendMode::Normal,
            },
        );
        l.commands.insert(3, DrawCmd::Effect { effect: 2 });
        l.commands.push(DrawCmd::Restore);
        if name == "background-blur" {
            l.resources[1] = Resource::Paint(RenderPaint::Solid([240, 240, 240, 30]));
            l.resources
                .push(Resource::Paint(RenderPaint::Solid([20, 180, 50, 255])));
            l.resources.push(Resource::Shape(Shape::Rect {
                size: [128., 256.],
                radii: [0.; 4],
            }));
            l.commands.splice(
                0..0,
                [DrawCmd::Path {
                    shape: 4,
                    paint: 3,
                    stroke: None,
                }],
            );
        }
        scenes.push((name.into(), l));
    }
    let mut l = base();
    l.commands = vec![
        DrawCmd::Save,
        DrawCmd::Transform([0.8, 0.2, -0.2, 0.8, 65., 45.]),
        DrawCmd::Clip { shape: 0 },
        DrawCmd::Layer {
            opacity: 0.5,
            blend: BlendMode::Normal,
        },
        DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        },
        DrawCmd::Transform([1., 0., 0., 1., 120., 120.]),
        DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        },
        DrawCmd::Restore,
        DrawCmd::Restore,
    ];
    scenes.push(("clip-opacity-transform".into(), l));
    scenes
}
pub fn many_nodes(count: usize, pan: f32, zoom: f32) -> DisplayList {
    let mut list = DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [8., 8.],
                radii: [1.; 4],
            }),
            Resource::Paint(RenderPaint::Solid([40, 120, 200, 255])),
        ],
        commands: vec![],
    };
    for i in 0..count {
        list.commands.extend([
            DrawCmd::Transform([
                zoom,
                0.,
                0.,
                zoom,
                (i % 100) as f32 * 10. * zoom + pan,
                (i / 100) as f32 * 10. * zoom,
            ]),
            DrawCmd::Path {
                shape: 0,
                paint: 1,
                stroke: None,
            },
        ]);
    }
    list
}
