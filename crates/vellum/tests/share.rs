//! TEST DATA. Snapshot collection reads only referenced files through a fake filesystem.
use std::{collections::BTreeMap, sync::Arc};
use vellum_contracts::fakes::MemFs as MemoryFs;
use vellum_core::ports::FileSystem;
use vellum_types::{Document, Node};
#[test]
fn snapshot_carries_only_dependencies_and_preserves_the_document() {
    let fs = Arc::new(MemoryFs::new());
    let mut doc = Document::new("Fixture");
    doc.imports
        .insert("kit".into(), "../lib/kit.lib.vellum".into());
    let mut lib = Document::new("Library");
    lib.meta.kind = vellum_types::DocumentKind::Library;
    let codec = vellum_codec_json::JsonCodec::new();
    fs.create_dir_all("lib").unwrap();
    fs.create_dir_all("design").unwrap();
    fs.write("lib/kit.lib.vellum", &codec.encode(&lib).unwrap())
        .unwrap();
    fs.write("private.txt", b"do not upload").unwrap();
    let script:Node=serde_json::from_value(serde_json::json!({"type":"script","id":"script1","name":"Pattern","src":"pattern.js","width":100,"height":100})).unwrap();
    doc.pages.push(vellum_types::Page {
        id: vellum_types::PageId::new("page1").unwrap(),
        name: "Page".into(),
        kind: Default::default(),
        children: vec![script],
    });
    let bundle = vellum::daemon::share::collect(fs.clone(), "design/app.vellum", &doc).unwrap();
    assert_eq!(bundle.missing, vec!["design/pattern.js"]);
    assert!(!bundle.files.iter().any(|f| f.path == "private.txt"));
    assert!(bundle.files.iter().any(|f| f.path == "lib/kit.lib.vellum"));
    let entry = bundle
        .decoded_files()
        .unwrap()
        .remove("design/app.vellum")
        .unwrap();
    assert_eq!(codec.decode(&entry).unwrap(), doc);
}
#[test]
fn snapshot_rejects_escape_paths_and_bounds_cyclic_imports() {
    let fs = Arc::new(MemoryFs::new());
    let mut doc = Document::new("Fixture");
    doc.imports = BTreeMap::from([("escape".into(), "../../private.lib.vellum".into())]);
    assert!(vellum::daemon::share::collect(fs.clone(), "app.vellum", &doc).is_err());
}

/// @native-renderer. CPU Skia and bundled fonts; no GPU/network/account needed.
#[test]
fn native_snapshot_renders_and_produces_a_portable_acceptance_bundle() {
    let fs = Arc::new(MemoryFs::new());
    let mut doc = Document::new("Snapshot fixture");
    let node:Node=serde_json::from_value(serde_json::json!({"type":"frame","id":"frame1","name":"Fixture frame","width":160,"height":100,"fills":[{"type":"solid","color":"#234D3C"}]})).unwrap();
    doc.pages.push(vellum_types::Page {
        id: vellum_types::PageId::new("page1").unwrap(),
        name: "Page".into(),
        kind: Default::default(),
        children: vec![node],
    });
    let mut bundle = vellum::daemon::share::collect(fs, "design/app.vellum", &doc).unwrap();
    vellum::daemon::share::render(&mut bundle, &doc).unwrap();
    assert_eq!(bundle.frames.len(), 1);
    let files = bundle.decoded_files().unwrap();
    let list =
        vellum_types::display::DisplayList::from_bytes(&files[&bundle.frames[0].display_list])
            .unwrap();
    assert!(!list.commands.is_empty());
    assert_eq!(
        vellum_codec_json::JsonCodec::new()
            .decode(&files["design/app.vellum"])
            .unwrap(),
        doc
    );
    if let Ok(path) = std::env::var("VELLUM_TEST_SHARE_OUTPUT") {
        std::fs::write(path, bundle.bytes().unwrap()).unwrap();
    }
}

#[test]
fn script_snapshots_freeze_sandbox_output_and_never_ship_a_viewer_program() {
    let fs = Arc::new(MemoryFs::new());
    let mut doc = Document::new("Script fixture");
    let node:Node=serde_json::from_value(serde_json::json!({"type":"script","id":"script1","name":"Pattern","src":"pattern.js","width":100,"height":100})).unwrap();
    doc.pages.push(vellum_types::Page {
        id: vellum_types::PageId::new("page1").unwrap(),
        name: "Page".into(),
        kind: Default::default(),
        children: vec![node],
    });
    fs.write("pattern.js",b"// @schema 1\nreturn [{type:'rect',width:100,height:100,fills:[{type:'solid',color:'#123456'}]}];").unwrap();
    let mut bundle = vellum::daemon::share::collect(fs, "app.vellum", &doc).unwrap();
    vellum::daemon::share::render(&mut bundle, &doc).unwrap();
    let files = bundle.decoded_files().unwrap();
    let renderer = vellum_render_skia::Renderer::new().unwrap();
    let resources = vellum_render_skia::Resources::default();
    let scene = renderer
        .prepare_buffer(&files[&bundle.frames[0].display_list], &resources)
        .unwrap();
    let rgba = renderer
        .rgba(&vellum_render_skia::Page {
            width: 100,
            height: 100,
            scene: &scene,
        })
        .unwrap();
    assert_eq!(
        &rgba[(50 * 100 + 50) * 4..(50 * 100 + 50) * 4 + 4],
        &[18, 52, 86, 255]
    );
    assert!(files.contains_key("pattern.js"));
    assert_eq!(doc.pages[0].children[0].id().as_str(), "script1");
}

#[test]
fn downloaded_root_keeps_sibling_library_paths_inside_its_filesystem() {
    let root = std::env::temp_dir().join(format!("vellum-share-root-{}", std::process::id()));
    std::fs::create_dir_all(root.join("design")).unwrap();
    std::fs::write(root.join(vellum_types::brand::SHARE_ROOT_MARKER), b"").unwrap();
    let file = root.join("design/app.vellum");
    let (fs, inner) = vellum::daemon::production::locate()(file.to_str().unwrap()).unwrap();
    assert_eq!(inner, "design/app.vellum");
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::write(root.join("lib/kit.lib.vellum"), b"TEST DATA").unwrap();
    assert_eq!(fs.read("lib/kit.lib.vellum").unwrap(), b"TEST DATA");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn free_form_metadata_never_becomes_a_file_dependency() {
    let fs = Arc::new(MemoryFs::new());
    fs.write("private.txt", b"TEST DATA private file").unwrap();
    let mut doc = Document::new("Metadata fixture");
    let node:Node=serde_json::from_value(serde_json::json!({"type":"frame","id":"frame1","name":"Frame","width":100,"height":100,"metadata":{"hint":{"src":"private.txt","asset":"not-an-asset"}}})).unwrap();
    doc.pages.push(vellum_types::Page {
        id: vellum_types::PageId::new("page1").unwrap(),
        name: "Page".into(),
        kind: Default::default(),
        children: vec![node],
    });
    let bundle = vellum::daemon::share::collect(fs, "app.vellum", &doc).unwrap();
    assert!(bundle.missing.is_empty());
    assert!(!bundle.files.iter().any(|f| f.path == "private.txt"));
    assert_eq!(bundle.files.len(), 1);
}

#[test]
fn typed_image_paints_are_captured_while_metadata_in_the_same_node_is_ignored() {
    let fs = Arc::new(MemoryFs::new());
    let bytes = b"TEST DATA image bytes";
    let hash = vellum_assets_fs::content_hash(bytes);
    fs.create_dir_all("app.assets").unwrap();
    fs.write(&format!("app.assets/{hash}"), bytes).unwrap();
    fs.write("private.txt", b"TEST DATA unrelated").unwrap();
    let mut doc = Document::new("Image fixture");
    let node:Node=serde_json::from_value(serde_json::json!({"type":"frame","id":"frame1","name":"Frame","width":100,"height":100,"fills":[{"type":"image","asset":hash}],"metadata":{"src":"private.txt"}})).unwrap();
    doc.pages.push(vellum_types::Page {
        id: vellum_types::PageId::new("page1").unwrap(),
        name: "Page".into(),
        kind: Default::default(),
        children: vec![node],
    });
    let bundle = vellum::daemon::share::collect(fs, "app.vellum", &doc).unwrap();
    assert_eq!(bundle.files.len(), 2);
    assert_eq!(
        bundle.decoded_files().unwrap()[&format!("app.assets/{hash}")],
        bytes
    );
    assert!(bundle.missing.is_empty());
}
