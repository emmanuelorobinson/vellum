//! TEST DATA: four catalog examples through real native compiler, script runtime, resolver and renderer.
mod common;
use std::{collections::BTreeMap, sync::Arc};
use vellum_contracts::fakes::SequentialIds;
use vellum_core::{ports::*, resolver::variables::Theme, script_nodes};
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::{display::*, *};
fn shader_list(src: &str, effect: bool, uniforms: BTreeMap<String, RenderUniform>) -> DisplayList {
    let mut list = DisplayList {
        resources: vec![
            Resource::Shape(Shape::Rect {
                size: [16., 16.],
                radii: [0.; 4],
            }),
            Resource::Paint(RenderPaint::Solid([255; 4])),
        ],
        commands: vec![
            DrawCmd::Transform([1., 0., 0., 1., 0., 0.]),
            DrawCmd::Layer {
                opacity: 1.,
                blend: BlendMode::Normal,
            },
        ],
    };
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
fn catalog_shader_goldens() {
    for (id, code, effect, uniforms, expected) in [
        (
            "checker",
            include_str!("../../../examples/shaders/checker.vsh"),
            false,
            BTreeMap::from([
                ("tint".into(), RenderUniform::Color([64, 64, 64, 255])),
                ("scale".into(), RenderUniform::Number(4.)),
            ]),
            include_bytes!("../../../examples/goldens/checker.rgba").as_slice(),
        ),
        (
            "tint-content",
            include_str!("../../../examples/shaders/tint-content.vsh"),
            true,
            BTreeMap::from([
                ("tint".into(), RenderUniform::Color([255, 0, 0, 255])),
                ("strength".into(), RenderUniform::Number(1.)),
            ]),
            include_bytes!("../../../examples/goldens/tint-content.rgba").as_slice(),
        ),
    ] {
        vellum_shader_sksl::SkslCompiler
            .compile(code, ShaderTarget::Sksl)
            .unwrap();
        let resources = Resources {
            shaders: BTreeMap::from([(id.into(), Arc::from(code))]),
            ..Default::default()
        };
        let renderer = Renderer::new().unwrap();
        let scene = renderer
            .prepare(&shader_list(id, effect, uniforms.clone()), &resources)
            .unwrap();
        assert!(
            scene.shader_errors().is_empty(),
            "{:?}",
            scene.shader_errors()
        );
        let page = Page {
            width: 16,
            height: 16,
            scene: &scene,
        };
        assert_eq!(renderer.rgba(&page).unwrap(), expected, "{id}");
        std::fs::create_dir_all("../../target/shader-examples").unwrap();
        std::fs::write(
            format!("../../target/shader-examples/{id}.bin"),
            shader_list(id, effect, uniforms).to_bytes().unwrap(),
        )
        .unwrap();
        std::fs::write(
            format!("../../target/shader-examples/{id}.png"),
            renderer.png(&page).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn script_and_design_system_goldens() {
    let services = vellum::daemon::production::scene_services();
    let mut doc: Document = serde_json::from_str(include_str!(
        "../../../examples/design-systems/starter.lib.vellum"
    ))
    .unwrap();
    let frame = services
        .prepare_export(&doc, &["swatch".into()])
        .unwrap()
        .frames
        .remove(0);
    let renderer = Renderer::new().unwrap();
    let render = |list: &DisplayList| {
        let scene = renderer.prepare(list, &Resources::default()).unwrap();
        renderer
            .rgba(&Page {
                width: 16,
                height: 16,
                scene: &scene,
            })
            .unwrap()
    };
    assert_eq!(
        render(&frame.output.list),
        include_bytes!("../../../examples/goldens/starter.rgba").as_slice()
    );
    let node:Node=serde_json::from_value(serde_json::json!({"type":"script","id":"bars","name":"TEST DATA","src":"bars.js","width":16,"height":16})).unwrap();
    let out = script_nodes::evaluate(
        &vellum_script_quickjs::QuickJsRuntime::new(|| 0, 1),
        &doc,
        &node,
        include_str!("../../../examples/scripts/bars.js"),
        &Theme::defaults(&doc),
        &mut SequentialIds::new(),
    )
    .unwrap();
    doc.pages[0].children = out.nodes;
    let frame = services
        .prepare_export(&doc, &["page".into()])
        .unwrap()
        .frames
        .remove(0);
    // Exported page bounds rebase to the content; retain the script's 16x16 viewport for its golden.
    let mut list = frame.output.list;
    for cmd in &mut list.commands {
        if let DrawCmd::Transform(m) = cmd {
            m[5] += 8.;
        }
    }
    assert_eq!(
        render(&list),
        include_bytes!("../../../examples/goldens/bars.rgba").as_slice()
    );
}
#[test]
fn shader_files_variables_screenshot_and_undo_share_the_write_and_render_paths() {
    use serde_json::json;
    use vellum::tools::{Registry, Session, ToolCtx};
    let doc:Document=serde_json::from_value(json!({
        "meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},
        "variables":{
            "tint":{"type":"color","name":"color/tint","values":[{"value":"#404040"}]},
            "scale":{"type":"number","name":"scale","values":[{"value":4}]}
        },
        "pages":[{"id":"page","name":"Page","children":[{
            "id":"rect","type":"rect","name":"TEST DATA","width":16,"height":16,
            "fills":[{"type":"shader","src":"effects/checker.vsh","uniforms":{"tint":{"$var":"tint"},"scale":{"$var":"scale"}}}]
        }]}]
    })).unwrap();
    let mut daemon = common::Daemon::with_scene(&doc, &["app.vellum"]);
    daemon.fs.seed(
        "effects/checker.vsh",
        include_bytes!("../../../examples/shaders/checker.vsh"),
    );
    let scene = daemon
        .docs
        .export_scene("app.vellum", &doc, &["rect".into()])
        .unwrap();
    let png = daemon
        .docs
        .services()
        .render_export(&scene.frames, "png")
        .unwrap();
    let renderer = Renderer::new().unwrap();
    let scene_pixels = renderer
        .prepare(
            &scene.frames[0].output.list,
            &Resources {
                shaders: scene.frames[0].shaders.clone(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        renderer
            .rgba(&Page {
                width: 16,
                height: 16,
                scene: &scene_pixels
            })
            .unwrap(),
        include_bytes!("../../../examples/goldens/checker.rgba").as_slice()
    );
    let mut session = Session::default();
    let registry = Registry::standard();
    registry
        .call(
            &mut ToolCtx::new(&mut daemon.docs, &mut session),
            "open_document",
            json!({"path":"app.vellum"}),
        )
        .unwrap();
    let shot = registry
        .call(
            &mut ToolCtx::new(&mut daemon.docs, &mut session),
            "screenshot",
            json!({"node":"rect"}),
        )
        .unwrap();
    assert_eq!(shot["image"]["base64"], vellum_core::base64::encode(&png));
    let person = Actor::Person {
        device: DeviceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap(),
    };
    let mut store = vellum_contracts::fakes::InMemoryDocStore::new(&doc, 1);
    let patch:NodePatch=serde_json::from_value(json!({"fills":[{"type":"shader","src":"effects/checker.vsh","uniforms":{"tint":"#FF0000","scale":2}}]})).unwrap();
    vellum_core::ops::apply(
        &mut store,
        &mut SequentialIds::new(),
        vellum_core::ops::Transaction::new(
            Origin {
                actor: person.clone(),
                session: None,
                turn: None,
            },
            vec![vellum_core::ops::Op::Update {
                id: NodeId::new("rect").unwrap(),
                patch,
            }],
        ),
    )
    .unwrap();
    assert_ne!(store.snapshot(), doc);
    store.undo(&person).unwrap();
    assert_eq!(store.snapshot(), doc);
    daemon.fs.seed(
        "effects/checker.vsh",
        b"half4 main(float2 p) { while(true){} return half4(1); }",
    );
    assert!(
        daemon
            .docs
            .export_scene("app.vellum", &doc, &["rect".into()])
            .is_err()
    );
}
