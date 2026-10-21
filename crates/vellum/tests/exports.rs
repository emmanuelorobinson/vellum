//! TEST DATA. Export tool over real scene services; no account, file writes or model calls.
mod common;
use serde_json::{Value, json};
use std::io::{Cursor, Read};
use vellum::{
    daemon::production::scene_services,
    tools::{ErrorKind, Registry, Session, ToolCtx},
};
use vellum_core::{doc_api::Services, export::*, ports::*};
fn doc() -> vellum_types::Document {
    serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA: export","createdWith":"Vellum 0.0.0"},"pages":[{"id":"page000001","name":"Page","children":[
        {"type":"frame","id":"frame00001","name":"First","width":64,"height":40,"fills":[{"type":"solid","color":"#FF0000"}],"children":[{"type":"rect","id":"rect000001","name":"Inset","position":{"type":"absolute","x":8,"y":8},"width":16,"height":16,"fills":[{"type":"solid","color":"#00FF00"}]}]},
        {"type":"frame","id":"frame00002","name":"Second","position":{"type":"absolute","x":100,"y":0},"width":32,"height":48,"fills":[{"type":"solid","color":"#0000FF"}]},
        {"type":"rect","id":"overlap001","name":"Sibling","width":64,"height":40,"fills":[{"type":"solid","color":"#FFFFFF"}]}
    ]}]})).unwrap()
}
fn call(input: Value) -> Result<Value, vellum::tools::ToolError> {
    let mut daemon = common::Daemon::with_scene(&doc(), &["export.vellum"]);
    let mut session = Session::default();
    let registry = Registry::standard();
    let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
    registry
        .call(&mut ctx, "open_document", json!({"path":"export.vellum"}))
        .unwrap();
    registry.call(&mut ctx, "export", input)
}
#[test]
fn export_tool_returns_copyable_html_and_real_image_formats_without_writing() {
    for format in [
        "html-css",
        "html-tailwind",
        "png",
        "jpeg",
        "webp",
        "svg",
        "pdf",
    ] {
        let out = call(json!({"nodes":["frame00001"],"format":format})).unwrap();
        assert_eq!(out["files"].as_array().unwrap().len(), 1, "{format}");
        assert!(out["files"][0]["base64"].as_str().unwrap().len() > 30);
    }
    let out = call(json!({"nodes":["frame00002","frame00001"],"format":"pdf"})).unwrap();
    assert_eq!(out["files"][0]["mime"], "application/pdf");
    let out = call(json!({"nodes":["frame00001","frame00002"],"format":"png","scale":3})).unwrap();
    assert_eq!(out["files"][0]["mime"], "application/zip");
}
#[test]
fn invalid_requests_and_missing_nodes_fail_explicitly() {
    for input in [
        json!({"nodes":[],"format":"png"}),
        json!({"nodes":["frame00001"],"format":"png","scale":4}),
        json!({"nodes":["frame00001"],"format":"react"}),
    ] {
        assert_eq!(call(input).unwrap_err().kind(), ErrorKind::InvalidInput);
    }
    assert_eq!(
        call(json!({"nodes":["missing001"],"format":"png"}))
            .unwrap_err()
            .kind(),
        ErrorKind::NotFound
    );
}
#[test]
fn selection_is_isolated_and_pdf_keeps_selection_order() {
    let services = scene_services();
    let scene = services
        .prepare_export(&doc(), &["frame00001".into()])
        .unwrap();
    assert!(
        !scene.frames[0]
            .output
            .painted
            .iter()
            .any(|id| id.last().as_str() == "overlap001")
    );
    let bytes = services.render_export(&scene.frames, "png").unwrap();
    assert!(bytes.starts_with(b"\x89PNG"));
    let scene = services
        .prepare_export(&doc(), &["frame00002".into(), "frame00001".into()])
        .unwrap();
    let pdf = services.render_export(&scene.frames, "pdf").unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/Count 2"));
    assert!(
        text.find("/MediaBox [0 0 32 48]").unwrap() < text.find("/MediaBox [0 0 64 40]").unwrap()
    );
}
#[test]
fn archive_entries_have_unique_safe_names_and_correct_dimensions() {
    struct Host(std::rc::Rc<dyn Services>);
    impl ExportRenderer for Host {
        fn render(&self, f: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError> {
            self.0.render_export(f, format)
        }
    }
    let services = scene_services();
    let mut scene = services
        .prepare_export(&doc(), &["frame00001".into(), "frame00002".into()])
        .unwrap();
    for f in &mut scene.frames {
        f.name = "../../same".into();
    }
    let result = vellum_export_raster::Images::new("png")
        .unwrap()
        .export(
            &scene,
            &ImageOptions {
                scale: 2,
                suffix: "/../../x".into(),
            },
            &Host(services),
        )
        .unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(&result.files[0].bytes)).unwrap();
    assert_eq!(zip.len(), 2);
    for i in 0..2 {
        let mut file = zip.by_index(i).unwrap();
        assert!(!file.name().contains('/'));
        let mut b = vec![];
        file.read_to_end(&mut b).unwrap();
        assert_eq!(
            u32::from_be_bytes(b[16..20].try_into().unwrap()),
            if i == 0 { 128 } else { 64 }
        );
    }
}

#[test]
fn exports_read_only_referenced_canvas_assets_and_custom_fonts() {
    use vellum_assets_fs::{FsAssets, assets_dir_for};
    let mut d = doc();
    let daemon = common::Daemon::with_scene(&d, &["export.vellum"]);
    let assets = FsAssets::new(daemon.fs.clone(), assets_dir_for("export.vellum")).unwrap();
    // A real PNG produced by the same full renderer, then made into a content-addressed canvas asset.
    let services = scene_services();
    let scene = services.prepare_export(&d, &["frame00001".into()]).unwrap();
    let png = services.render_export(&scene.frames, "png").unwrap();
    let hash = assets.put(&png, "image/png").unwrap();
    d.pages[0].children[0]=serde_json::from_value(json!({"type":"image","id":"image00001","name":"Photo","asset":hash.as_str(),"width":64,"height":40})).unwrap();
    // Keep the original filesystem because it owns the assets; put the updated document into it.
    daemon.fs.seed("export.vellum", &common::encode(&d));
    let scene = daemon
        .docs
        .export_scene("export.vellum", &d, &["image00001".into()])
        .unwrap();
    assert_eq!(scene.frames[0].images.len(), 1);
    let bytes = services.render_export(&scene.frames, "png").unwrap();
    assert!(bytes.starts_with(b"\x89PNG"));
}

#[test]
fn custom_text_exports_embed_the_actual_font_and_clipboard_html_is_self_contained() {
    let mut d = doc();
    let daemon = common::Daemon::with_scene(&d, &["export.vellum"]);
    let assets = vellum_assets_fs::FsAssets::new(daemon.fs.clone(), "export.assets").unwrap();
    let hash = assets
        .put(
            include_bytes!("../../adapters/text-parley/fonts/InterVariable.ttf"),
            "font/ttf",
        )
        .unwrap();
    d.fonts =
        serde_json::from_value(json!({hash.as_str():{"family":"TEST DATA Font","weight":400}}))
            .unwrap();
    d.pages[0].children[0]=serde_json::from_value(json!({"type":"text","id":"customtext","name":"Custom text","width":180,"height":40,"fills":[{"type":"solid","color":"#112233"}],"content":[{"text":"<tag> & text","font":"TEST DATA Font","size":20,"lineHeight":28}]})).unwrap();
    daemon.fs.seed("export.vellum", &common::encode(&d));
    let scene = daemon
        .docs
        .export_scene("export.vellum", &d, &["customtext".into()])
        .unwrap();
    struct Host;
    impl ExportRenderer for Host {
        fn render(&self, _: &[ExportFrame], _: &str) -> Result<Vec<u8>, ExportError> {
            panic!("plain text should be DOM")
        }
    }
    let result = vellum_export_html::Html::new("html-tailwind")
        .unwrap()
        .export(&scene, &CodeOptions::default(), &Host)
        .unwrap();
    let html = result.text().unwrap();
    assert!(html.contains("@font-face"));
    assert!(html.contains("data:font/ttf;base64,"));
    assert!(html.contains("&lt;tag&gt; &amp; text"));
    let copy = result.clipboard().unwrap();
    assert_eq!(copy["text/html"], result.files[0].bytes);
    assert_eq!(copy["text/plain"], result.files[0].bytes);
}

#[test]
fn export_reads_the_live_unsaved_snapshot_and_changes_no_document_or_file() {
    use vellum::daemon::EditorId;
    let disk = doc();
    let mut daemon = common::Daemon::with_scene(&disk, &["export.vellum"]);
    let mut live = disk.clone();
    live.pages[0].children[0]=serde_json::from_value(json!({"type":"rect","id":"unsaved001","name":"Unsaved","width":12,"height":8,"fills":[{"type":"solid","color":"#AABBCC"}]})).unwrap();
    let editor = common::FakeEditor::new(&live);
    daemon
        .docs
        .open_editor("export.vellum", EditorId("editor1".into()), editor.link())
        .unwrap();
    let before = daemon.fs.mutations();
    let mut session = Session::default();
    let registry = Registry::standard();
    let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
    let out = registry
        .call(
            &mut ctx,
            "export",
            json!({"document":"export.vellum","nodes":["unsaved001"],"format":"png"}),
        )
        .unwrap();
    assert_eq!(out["files"][0]["mime"], "image/png");
    assert_eq!(editor.doc(), live);
    assert_eq!(daemon.on_disk("export.vellum"), disk);
    assert_eq!(daemon.fs.mutations(), before);
}

#[test]
fn a_text_frame_can_be_exported_through_the_bounded_tool() {
    let mut d = doc();
    d.pages[0].children[0]=serde_json::from_value(json!({"type":"text","id":"text000001","name":"Text","width":160,"height":40,"fills":[{"type":"solid","color":"#112233"}],"content":[{"text":"Design where code lives","font":"Inter","size":16}]})).unwrap();
    let mut daemon = common::Daemon::with_scene(&d, &["export.vellum"]);
    let mut session = Session::default();
    let registry = Registry::standard();
    let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
    let out = registry
        .call(
            &mut ctx,
            "export",
            json!({"document":"export.vellum","nodes":["text000001"],"format":"html-css"}),
        )
        .unwrap();
    assert!(serde_json::to_vec(&out).unwrap().len() < 950 * 1024);
    assert!(out["clipboard"]["text/html"].is_string());
}

#[test]
fn html_text_layers_keep_ink_and_lines_outside_the_text_box() {
    let mut d = doc();
    d.pages[0].children[0]=serde_json::from_value(json!({"type":"text","id":"text000001","name":"Overflow","width":160,"height":5,"fills":[{"type":"solid","color":"#112233"}],"content":[{"text":"jumps\nbelow","font":"Inter","size":16,"lineHeight":24}]})).unwrap();
    let scene = scene_services()
        .prepare_export(&d, &["text000001".into()])
        .unwrap();
    let visual = scene.frames[0].text_visuals.values().next().unwrap();
    assert!(visual.bounds.x < 0. && visual.bounds.y < 0.);
    assert!(visual.bounds.height >= 48.);
    assert!(visual.bytes.starts_with(b"\x89PNG"));
}

#[test]
fn library_local_ids_do_not_shadow_a_selected_document_layer() {
    let mut d = doc();
    d.imports.insert("ui".into(), "ui.lib.vellum".into());
    d.pages[0].children.insert(0,serde_json::from_value(json!({"type":"instance","id":"instance01","name":"Instance","componentId":"component1","library":"ui","width":30,"height":30})).unwrap());
    let daemon = common::Daemon::with_scene(&d, &["export.vellum"]);
    let lib:vellum_types::Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA: library","createdWith":"Vellum 0.0.0","kind":"library"},"pages":[],"components":{"component1":{"name":"Tile","root":{"type":"frame","id":"libRoot001","name":"Tile","width":30,"height":30,"children":[{"type":"rect","id":"frame00001","name":"Library-local id","width":9,"height":11}]}}}})).unwrap();
    daemon.fs.seed("ui.lib.vellum", &common::encode(&lib));
    let scene = daemon
        .docs
        .export_scene("export.vellum", &d, &["frame00001".into()])
        .unwrap();
    assert_eq!(scene.frames[0].bounds.width, 64.);
    assert_eq!(scene.frames[0].roots[0].address.to_string(), "frame00001");
}

#[test]
fn slides_pdf_expands_pages_orders_subsets_and_keeps_frame_sizes() {
    let mut d = doc();
    d.pages[0].kind = vellum_types::PageKind::Slides;
    d.pages[0].children.pop();
    // Annotations never become PDF pages.
    d.pages[0].children.insert(1, serde_json::from_value(json!({"type":"note","id":"notes00001","name":"Private notes","content":"TEST DATA","width":100,"height":100})).unwrap());
    let before = common::encode(&d);
    let mut daemon = common::Daemon::with_scene(&d, &["slides.vellum"]);
    let mut session = Session::default();
    let registry = Registry::standard();
    for ids in [json!(["page000001"]), json!(["frame00002", "frame00001"])] {
        let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
        let out = registry
            .call(
                &mut ctx,
                "export",
                json!({"document":"slides.vellum","nodes":ids,"format":"pdf"}),
            )
            .unwrap();
        assert_eq!(out["nodes"], json!(["frame00001", "frame00002"]));
        let bytes =
            vellum_core::base64::decode(out["files"][0]["base64"].as_str().unwrap()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/Count 2"));
        assert!(
            text.find("/MediaBox [0 0 64 40]").unwrap()
                < text.find("/MediaBox [0 0 32 48]").unwrap()
        );
    }
    let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
    let out = registry
        .call(
            &mut ctx,
            "export",
            json!({"document":"slides.vellum","nodes":["frame00002"],"format":"pdf"}),
        )
        .unwrap();
    let bytes = vellum_core::base64::decode(out["files"][0]["base64"].as_str().unwrap()).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Count 1"));
    assert!(text.contains("/MediaBox [0 0 32 48]"));
    assert_eq!(common::encode(&daemon.on_disk("slides.vellum")), before);
}

#[test]
fn slides_pdf_preserves_fractional_frame_dimensions() {
    let mut d = doc();
    d.pages[0].kind = vellum_types::PageKind::Slides;
    d.pages[0].children.truncate(1);
    if let vellum_types::Node::Frame(frame) = &mut d.pages[0].children[0] {
        frame.width = serde_json::from_value(json!(64.5)).unwrap();
        frame.height = serde_json::from_value(json!(40.25)).unwrap();
    }
    let scene = scene_services()
        .prepare_export(&d, &["frame00001".into()])
        .unwrap();
    let pdf = scene_services()
        .render_export(&scene.frames, "pdf")
        .unwrap();
    if let Ok(dir) = std::env::var("VELLUM_SLIDE_ARTIFACTS") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(format!("{dir}/fractional.pdf"), &pdf).unwrap();
        let doubled: Vec<_> = scene.frames.iter().map(|f| f.at_scale(2.)).collect();
        let png = scene_services().render_export(&doubled, "png").unwrap();
        std::fs::write(format!("{dir}/fractional.png"), png).unwrap();
    }
    assert!(
        String::from_utf8_lossy(&pdf)
            .replace("/MediaBox[", "/MediaBox [")
            .contains("/MediaBox [0 0 64.5 40.25]")
    );
}
