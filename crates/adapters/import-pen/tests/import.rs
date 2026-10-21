//! TEST DATA. Hand-authored interchange fixtures, not copied vendor designs or schemas.
use serde_json::{Value, json};
use vellum_core::ports::{ImportInput, ImportedAsset, Importer};
use vellum_import_pen::PenImporter;
fn input(bytes: &[u8]) -> ImportInput<'_> {
    ImportInput {
        bytes,
        name: "TEST DATA.pen",
    }
}
#[test]
fn editable_layers_keep_layout_text_strokes_and_resolve_inherited_themes() {
    let bytes=serde_json::to_vec(&json!({"version":"2.21","themes":{"appearance":["light","dark"]},"variables":{"ink":{"type":"color","value":[{"value":"#123456","theme":{"appearance":"light"}},{"value":"#FEDCBA","theme":{"appearance":"dark"}}]},"space":{"type":"number","value":8}},"children":[{"type":"frame","id":"source-card","name":"TEST DATA card","x":15,"y":25,"width":320,"height":"fit_content(100)","theme":{"appearance":"dark"},"layout":"vertical","padding":[4,8],"gap":"$space","fill":"$ink","stroke":["#AABBCC",{"type":"color","color":"$ink"}],"strokeWidth":{"top":1,"right":2,"bottom":3,"left":4},"children":[{"type":"text","id":"label","content":"TEST DATA title","fontFamily":"Inter","fontSize":16,"fontWeight":"600","lineHeight":1.5,"textGrowth":"fixed-width","width":"fill_container","fill":"$ink"}]}]})).unwrap();
    let r = PenImporter::default().import(&input(&bytes)).unwrap();
    let n = &r.nodes[0];
    assert_eq!(n["layout"], "column");
    assert_eq!(n["gap"], 8.0);
    assert_eq!(n["height"]["hug"], 100.0);
    assert_eq!(n["fills"][0]["color"], "#FEDCBA");
    assert_eq!(n["stroke"]["paints"].as_array().unwrap().len(), 2);
    assert_eq!(n["stroke"]["width"][1], 2.0);
    assert_eq!(n["children"][0]["content"][0]["text"], "TEST DATA title");
    assert_eq!(n["children"][0]["content"][0]["lineHeight"], 24.0);
    assert!(r.notes.iter().any(|s| s.contains("resolved")));
}
#[test]
fn instances_expand_with_nested_overrides_without_executing_active_content() {
    let bytes=serde_json::to_vec(&json!({"version":"2.21","children":[{"type":"frame","id":"card","reusable":true,"width":100,"height":60,"layout":"none","children":[{"id":"text","type":"text","content":"TEST DATA before"}]},{"type":"ref","id":"copy","ref":"card","x":150,"descendants":{"text":{"content":"TEST DATA after"}}},{"type":"script","id":"script","width":100,"height":100,"scriptUri":"./never.js"},{"type":"prompt","id":"prompt","content":"TEST DATA inert instruction"}]})).unwrap();
    let r = PenImporter::default().import(&input(&bytes)).unwrap();
    assert_eq!(r.nodes.len(), 3);
    assert_eq!(
        r.nodes[1]["children"][0]["content"][0]["text"],
        "TEST DATA after"
    );
    assert_eq!(r.nodes[1]["position"]["x"], 150.0);
    assert_eq!(r.nodes[2]["type"], "text");
    assert!(r.notes.iter().any(|s| s.contains("script")));
    assert!(!format!("{:?}", r.nodes).contains("never.js"));
}
#[test]
fn companion_images_are_explicit_data_and_unsafe_urls_are_never_requested() {
    let bytes=serde_json::to_vec(&json!({"version":"2.21","children":[{"id":"picture","type":"rectangle","width":100,"height":100,"fill":{"type":"image","url":"./images/test.png","mode":"contain"}},{"id":"bad","type":"rectangle","width":100,"height":100,"fill":{"type":"image","url":"../../private.png"}}]})).unwrap();
    let paths = PenImporter::image_paths(&input(&bytes)).unwrap();
    assert_eq!(paths, vec!["images/test.png"]);
    let importer = PenImporter::with_assets(
        [(
            String::from("images/test.png"),
            ImportedAsset {
                mime: "image/png".into(),
                bytes: b"TEST DATA image".to_vec(),
            },
        )]
        .into(),
    );
    let r = importer.import(&input(&bytes)).unwrap();
    assert_eq!(r.assets.len(), 1);
    assert_eq!(r.nodes[0]["fills"][0]["asset"], "import:0");
    assert_eq!(r.nodes[0]["fills"][0]["fit"], "fit");
    assert!(r.notes.iter().any(|s| s.contains("image")));
}
#[test]
fn comments_are_supported_but_versions_cycles_and_invalid_values_fail_explicitly() {
    let valid=b"{ // TEST DATA\n\"version\":\"2.21\",\"children\":[{\"id\":\"rect\",\"type\":\"rectangle\",\"width\":10,\"height\":20}]}";
    vellum_contracts::suites::import::importer(&PenImporter::default(), valid);
    for v in [
        json!({"version":"2.22","children":[]}),
        json!({"version":"2.21","children":[{"id":"cycle","type":"ref","ref":"cycle"}]}),
        json!({"version":"2.21","variables":{"bad":{"type":"number","value":"$bad"}},"children":[{"id":"rect","type":"rectangle","width":"$bad","height":10}]}),
        json!({"version":"2.21","children":[{"id":"rect","type":"rectangle","width":-1,"height":10}]}),
    ] {
        assert!(
            PenImporter::default()
                .import(&input(&serde_json::to_vec(&v).unwrap()))
                .is_err()
        );
    }
}
#[test]
fn output_is_valid_vellum_content_after_host_assigns_ids() {
    let bytes=serde_json::to_vec(&json!({"version":"2.21","children":[{"id":"group","type":"frame","layout":"none","width":100,"height":100,"children":[{"id":"rect","type":"rectangle","width":20,"height":30,"rotation":30,"cornerRadius":[1,2,3,4],"fill":"#123456","effect":{"type":"shadow","offset":{"x":2,"y":3},"blur":4,"color":"#00000080"}}]}]})).unwrap();
    let mut r = PenImporter::default().import(&input(&bytes)).unwrap();
    fn ids(v: &mut Value, n: &mut usize) {
        *n += 1;
        v["id"] = json!(format!("node{:06}", n));
        for child in v
            .get_mut("children")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            ids(child, n)
        }
    }
    ids(&mut r.nodes[0], &mut 0);
    let _: vellum_types::Node = serde_json::from_value(r.nodes.remove(0)).unwrap();
}

#[test]
fn nested_instance_overrides_and_child_replacements_survive_expansion() {
    let bytes=serde_json::to_vec(&json!({"version":"2.21","children":[
 {"id":"button","type":"frame","x":800,"width":80,"height":30,"children":[{"id":"label","type":"text","content":"TEST DATA base"}]},
 {"id":"card","type":"frame","width":100,"height":50,"children":[{"id":"nested","type":"ref","ref":"button","descendants":{"label":{"content":"TEST DATA inner"}}}]},
 {"id":"copy","type":"ref","ref":"card","descendants":{"nested/label":{"content":"TEST DATA outer"}}},
 {"id":"replacement","type":"ref","ref":"button","descendants":{"label":{"type":"rectangle","id":"new-child","width":12,"height":13,"fill":"#123456"}}}
 ]})).unwrap();
    let r = PenImporter::default().import(&input(&bytes)).unwrap();
    assert_eq!(
        r.nodes[2]["children"][0]["children"][0]["content"][0]["text"],
        "TEST DATA outer"
    );
    assert_eq!(r.nodes[3]["children"][0]["type"], "rect");
    assert_eq!(r.nodes[3]["position"]["x"], 0.0);
}
#[test]
fn cyclic_expansion_depth_duplicate_ids_and_unsafe_asset_paths_are_bounded() {
    for path in [
        "/private/a.png",
        "../a.png",
        "a/../b.png",
        "C:\\a.png",
        "https://example.com/a.png",
        "file:///private/a",
        "%2e%2e/a.png",
        "a.png?token=never",
    ] {
        assert!(vellum_import_pen::relative_image(path).is_none())
    }
    let mut child = json!({"type":"rectangle","id":"leaf","width":1,"height":1});
    for i in 0..66 {
        child = json!({"id":format!("parent-{i}"),"type":"frame","children":[child]})
    }
    assert!(
        PenImporter::default()
            .import(&input(
                &serde_json::to_vec(&json!({"version":"2.21","children":[child]})).unwrap()
            ))
            .is_err()
    );
    let duplicate = json!({"version":"2.21","children":[{"id":"same","type":"rectangle","width":1,"height":1},{"id":"same","type":"rectangle","width":1,"height":1}]});
    assert!(
        PenImporter::default()
            .import(&input(&serde_json::to_vec(&duplicate).unwrap()))
            .is_err()
    );
}

#[test]
fn gradients_mesh_and_hidden_graphics_produce_typed_nodes() {
    let doc = json!({"version":"2.21","children":[
    {"type":"rectangle","id":"gradient","width":100,"height":100,"fill":{"type":"gradient","gradientType":"linear","colors":[{"color":"#123","position":0},{"color":"#FEDCBA80","position":1}],"opacity":0.5}},
    {"type":"rectangle","id":"mesh","width":100,"height":100,"fill":{"type":"mesh_gradient","columns":2,"rows":2,"colors":["#123456","#123456","#123456","#123456"],"points":[[0,0],[1,0],[0,1],[1,1]]}},
    {"type":"rectangle","id":"hidden","width":100,"height":100,"fill":{"type":"color","color":"#123456","enabled":false},"effect":[{"type":"blur","radius":3},{"type":"background_blur","radius":4}]}
    ]});
    let r = PenImporter::default()
        .import(&input(&serde_json::to_vec(&doc).unwrap()))
        .unwrap();
    assert_eq!(r.nodes[0]["fills"][0]["stops"][0]["color"], "#11223380");
    for (i, mut n) in r.nodes.into_iter().enumerate() {
        n["id"] = json!(format!("node{:06}", i));
        let _: vellum_types::Node = serde_json::from_value(n).unwrap();
    }
}

#[test]
fn a_small_component_graph_cannot_amplify_text_past_the_output_budget() {
    let mut children = vec![json!({"id":"base","type":"text","content":"x".repeat(65536)})];
    for i in 0..1100 {
        children.push(json!({"id":format!("instance-{i}"),"type":"ref","ref":"base"}));
    }
    let source = serde_json::to_vec(&json!({"version":"2.21","children":children})).unwrap();
    assert!(source.len() < 256 * 1024);
    let error = PenImporter::default().import(&input(&source)).unwrap_err();
    assert!(error.to_string().contains("budget"));
}

#[test]
fn metadata_and_active_content_cannot_request_hidden_companion_reads() {
    let doc = json!({"version":"2.21","children":[{"id":"rect","type":"rectangle","width":1,"height":1,"metadata":{"type":"image","url":"./private.png"}},{"id":"script","type":"script","scriptUri":"./never.js","inputs":{"nested":{"type":"image","url":"./other.png"}}}]});
    assert!(
        PenImporter::image_paths(&input(&serde_json::to_vec(&doc).unwrap()))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn companion_discovery_uses_the_same_resolved_theme_as_the_import() {
    let doc = json!({"version":"2.21","themes":{"mode":["light","dark"]},"variables":{"picture":{"type":"string","value":[{"value":"./light.png","theme":{"mode":"light"}},{"value":"./dark.png","theme":{"mode":"dark"}}]}},"children":[{"id":"card","type":"frame","theme":{"mode":"dark"},"children":[{"id":"image","type":"rectangle","width":10,"height":10,"fill":{"type":"image","url":"$picture"}}]}]});
    assert_eq!(
        PenImporter::image_paths(&input(&serde_json::to_vec(&doc).unwrap())).unwrap(),
        vec!["dark.png"]
    );
}

#[test]
fn aliases_of_reusable_instances_keep_outer_descendant_overrides() {
    let doc = json!({"version":"2.21","children":[{"id":"base","type":"frame","width":100,"height":100,"children":[{"id":"label","type":"text","content":"TEST DATA base"}]},{"id":"alias","type":"ref","ref":"base","reusable":true,"descendants":{"label":{"content":"TEST DATA alias"}}},{"id":"instance","type":"ref","ref":"alias","descendants":{"label":{"content":"TEST DATA outer"}}}]});
    let r = PenImporter::default()
        .import(&input(&serde_json::to_vec(&doc).unwrap()))
        .unwrap();
    assert_eq!(
        r.nodes[2]["children"][0]["content"][0]["text"],
        "TEST DATA outer"
    );
}

#[test]
fn path_geometry_is_bounded_before_native_geometry_parsing() {
    let doc = json!({"version":"2.21","children":[{"id":"path","type":"path","width":10,"height":10,"viewBox":[0,0,10,10],"geometry":format!("M0 0{}"," ".repeat(1024*1024))}]});
    assert!(
        PenImporter::default()
            .import(&input(&serde_json::to_vec(&doc).unwrap()))
            .is_err()
    );
}
