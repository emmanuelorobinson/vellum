//! TEST DATA. The daemon's scene for one document (reports.md, Prompt 29): instances of a library the document
//! imports resolve from disk, so they lay out and draw.

mod common;

use common::Daemon;
use serde_json::json;
use vellum_core::ports::{FileSystem, ScriptValue};

const PATH: &str = "designs/app.vellum";

fn daemon() -> Daemon {
    let library = json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA library", "createdWith": "tests", "kind": "library"},
        "pages": [],
        "components": {"badge": {"name": "TEST DATA Badge", "root": {
            "type": "frame", "id": "badgeRoot", "name": "Badge", "width": 80, "height": 30,
            "fills": [{"type": "solid", "color": "#2878C8"}],
            "children": [{"type": "rect", "id": "badgeDot01", "name": "Dot", "width": 10, "height": 10,
                "position": {"type": "absolute", "x": 5, "y": 10}, "fills": [{"type": "solid", "color": "#FFFFFF"}]}]}}}
    });
    let doc: vellum_types::Document = serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA app", "createdWith": "tests"},
        "imports": {"ui": "library.lib.vellum"},
        "pages": [{"id": "page000001", "name": "TEST DATA", "children": [
            {"type": "instance", "id": "linked0001", "name": "Badge", "componentId": "badge", "library": "ui",
             "width": 80, "height": 30, "position": {"type": "absolute", "x": 10, "y": 10}}]}]
    }))
    .unwrap();
    let daemon = Daemon::with_scene(&doc, &[PATH]);
    daemon
        .fs
        .write(
            "designs/library.lib.vellum",
            &serde_json::to_vec(&library).unwrap(),
        )
        .unwrap();
    daemon
}

#[test]
fn an_instance_of_an_imported_library_is_laid_out_and_drawn() {
    let mut daemon = daemon();
    let doc = daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    let services = daemon.docs.services_for(PATH, &doc);
    let layout = services.layout(&doc, "page000001").expect("layout");
    assert!(
        layout
            .nodes
            .iter()
            .any(|n| n.node.to_string() == "linked0001/badgeDot01"),
        "the library component's tree is resolved: {:?}",
        layout
            .nodes
            .iter()
            .map(|n| n.node.to_string())
            .collect::<Vec<_>>()
    );
    let image = daemon.docs.render(PATH, "linked0001", 1.).expect("render");
    let ScriptValue::Map(image) = image else {
        panic!("an image")
    };
    assert_eq!(
        image.get("mime"),
        Some(&ScriptValue::Text("image/png".into()))
    );
    // The same document asks again: the services are reused, not rebuilt.
    let again = daemon.docs.services_for(PATH, &doc);
    assert!(std::rc::Rc::ptr_eq(&services, &again));
}

#[test]
fn without_a_document_scene_the_shared_one_reports_the_library_missing() {
    let mut daemon = daemon();
    let doc = daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    // The shared services have no document path, so the import cannot be found (the old behaviour).
    let shared = daemon.docs.services();
    assert!(
        shared.layout(&doc, "page000001").is_err() || {
            let l = shared.layout(&doc, "page000001").unwrap();
            !l.nodes
                .iter()
                .any(|n| n.node.to_string() == "linked0001/badgeDot01")
        }
    );
}
