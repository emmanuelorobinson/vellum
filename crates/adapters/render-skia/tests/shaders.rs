// TEST DATA. Native raster path, no GPU/network. Every gallery shader has an analytic pixel golden.
use std::{collections::BTreeMap, sync::Arc};
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::{BlendMode, display::*};
fn list(src: &str, effect: bool) -> DisplayList {
    let mut list = DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [16., 16.],
                radii: [0.; 4],
            }),
            Resource::Paint(RenderPaint::Solid([255, 0, 0, 255])),
        ],
        commands: vec![
            DrawCmd::Transform([1., 0., 0., 1., 0., 0.]),
            DrawCmd::Layer {
                opacity: 1.,
                blend: BlendMode::Normal,
            },
        ],
    };
    let uniforms = BTreeMap::from([("tint".into(), RenderUniform::Color([0, 255, 0, 255]))]);
    if effect {
        list.resources.push(Resource::Effect(RenderEffect::Shader {
            src: src.into(),
            uniforms,
        }));
        list.commands.push(DrawCmd::Effect { effect: 2 });
    } else {
        list.resources[1] = Resource::Paint(RenderPaint::Shader {
            src: src.into(),
            uniforms,
        });
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
#[test]
fn fill_effect_errors_and_cache_invalidation() {
    let renderer = Renderer::new().unwrap();
    let mut resources = Resources::default();
    resources.shaders.insert("fixture.vsh".into(),Arc::from("// @color tint\nuniform float4 tint; uniform float2 size; half4 main(float2 p) { return half4(tint); }"));
    let scene = renderer
        .prepare_buffer(&list("fixture.vsh", false).to_bytes().unwrap(), &resources)
        .unwrap();
    assert!(
        scene.shader_errors().is_empty(),
        "{:?}",
        scene.shader_errors()
    );
    let bytes = renderer
        .rgba(&Page {
            width: 16,
            height: 16,
            scene: &scene,
        })
        .unwrap();
    assert!(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 255, 0, 255])
    );
    resources.shaders.insert(
        "fixture.vsh".into(),
        Arc::from(
            "// @color tint\nuniform float4 tint; half4 main(float2 p) { return half4(0,0,1,1); }",
        ),
    );
    let scene = renderer
        .prepare_buffer(&list("fixture.vsh", false).to_bytes().unwrap(), &resources)
        .unwrap();
    assert_eq!(
        &renderer
            .rgba(&Page {
                width: 16,
                height: 16,
                scene: &scene
            })
            .unwrap()[..4],
        [0, 0, 255, 255]
    );
    resources.shaders.insert("fixture.vsh".into(),Arc::from("// @color tint\nuniform float4 tint; uniform shader content; half4 main(float2 p) { return content.eval(p) * half4(0,1,1,1); }"));
    let scene = renderer
        .prepare(&list("fixture.vsh", true), &resources)
        .unwrap();
    assert!(
        scene.shader_errors().is_empty(),
        "{:?}",
        scene.shader_errors()
    );
    assert_eq!(
        &renderer
            .rgba(&Page {
                width: 16,
                height: 16,
                scene: &scene
            })
            .unwrap()[..4],
        [0, 0, 0, 255]
    );
    resources.shaders.insert(
        "fixture.vsh".into(),
        Arc::from("half4 main(float2 p) { return missing; }"),
    );
    let scene = renderer
        .prepare(&list("fixture.vsh", false), &resources)
        .unwrap();
    assert_eq!(scene.shader_errors()[0].src, "fixture.vsh");
    assert!(matches!(
        scene.shader_errors()[0].error,
        vellum_core::ports::ShaderError::Invalid { .. }
    ));
    let scene = renderer
        .prepare(&list("absent.vsh", false), &resources)
        .unwrap();
    assert!(!scene.shader_errors().is_empty());
}
#[test]
fn boolean_vector_uniforms_and_failed_effect_preserve_typed_contracts() {
    let renderer = Renderer::new().unwrap();
    let resources = Resources {
        shaders: BTreeMap::from([(
            "typed".into(),
            Arc::from(
                "uniform int enabled; uniform float2 offset; half4 main(float2 p) { return half4(enabled != 0 ? offset.x : 0, offset.y, 0, 1); }",
            ),
        )]),
        ..Default::default()
    };
    let mut draw = list("typed", false);
    let Resource::Paint(RenderPaint::Shader { uniforms, .. }) = &mut draw.resources[1] else {
        panic!()
    };
    *uniforms = BTreeMap::from([
        ("enabled".into(), RenderUniform::Bool(true)),
        ("offset".into(), RenderUniform::Vector(vec![0.25, 0.5])),
    ]);
    let scene = renderer.prepare(&draw, &resources).unwrap();
    assert!(
        scene.shader_errors().is_empty(),
        "{:?}",
        scene.shader_errors()
    );
    assert_eq!(
        &renderer
            .rgba(&Page {
                width: 16,
                height: 16,
                scene: &scene
            })
            .unwrap()[..4],
        [64, 128, 0, 255]
    );
    let Resource::Paint(RenderPaint::Shader { uniforms, .. }) = &mut draw.resources[1] else {
        panic!()
    };
    uniforms.insert("offset".into(), RenderUniform::Vector(vec![1., 2., 3.]));
    assert_eq!(
        renderer
            .prepare(&draw, &resources)
            .unwrap()
            .shader_errors()
            .len(),
        1
    );
    let scene = renderer
        .prepare(&list("missing", true), &Resources::default())
        .unwrap();
    assert_eq!(scene.shader_errors().len(), 1);
    assert_eq!(
        &renderer
            .rgba(&Page {
                width: 16,
                height: 16,
                scene: &scene
            })
            .unwrap()[..4],
        [255, 0, 0, 255]
    );
}
#[test]
fn per_frame_source_and_draw_budgets_are_enforced() {
    let renderer = Renderer::new().unwrap();
    let resources = Resources {
        shaders: BTreeMap::from([("large".into(), Arc::from(" ".repeat(16385)))]),
        ..Default::default()
    };
    assert!(renderer.prepare(&list("large", false), &resources).is_err());
    let mut draw = list("missing", false);
    draw.commands.splice(
        2..2,
        (0..129).map(|_| DrawCmd::Path {
            shape: 0,
            paint: 1,
            stroke: None,
        }),
    );
    assert!(renderer.prepare(&draw, &Resources::default()).is_err());
}
