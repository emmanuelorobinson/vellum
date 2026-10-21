//! TEST DATA. Independently authored interchange inputs; no vendor designs or private SDKs.
mod common;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};
use vellum::{
    permissions::Outside,
    tools::{ErrorKind, Registry, Session, ToolCtx, ToolError},
};
use vellum_types::{Document, Origin};
fn document() -> Document {
    serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},"pages":[{"id":"page000001","name":"Page"}]})).unwrap()
}
fn run(
    d: &mut common::Daemon,
    s: &mut Session,
    name: &str,
    input: Value,
) -> Result<Value, ToolError> {
    Registry::standard().call(&mut ToolCtx::new(&mut d.docs, s), name, input)
}
fn fixture() -> String {
    serde_json::to_string(&json!({"version":"2.21","children":[{"id":"source-root","type":"frame","name":"TEST DATA card","width":200,"height":100,"layout":"none","children":[{"type":"rectangle","id":"picture","width":30,"height":40,"fill":{"type":"image","url":"./images/test.png"}},{"type":"text","id":"title","content":"TEST DATA","fontFamily":"Inter","fontSize":16,"fill":"#123456"}]}]})).unwrap()
}
#[test]
fn inline_pen_import_is_attributed_undoable_and_does_not_read_companions() {
    let original = document();
    let mut d = common::Daemon::new(&original, &["app.vellum"]);
    let mut s = Session::default();
    run(
        &mut d,
        &mut s,
        "open_document",
        json!({"path":"app.vellum"}),
    )
    .unwrap();
    let turn = vellum_types::TurnId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    s.set_turn(Some(turn.clone()));
    let r = run(
        &mut d,
        &mut s,
        "import",
        json!({"text":fixture(),"format":"pen","parent":"page000001"}),
    )
    .unwrap();
    assert_eq!(r["nodes"], 3);
    assert_eq!(r["importer"], "pen");
    assert!(
        r["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("images"))
    );
    let actor = vellum_types::Actor::Person {
        device: vellum_types::DeviceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap(),
    };
    d.docs
        .target("app.vellum")
        .unwrap()
        .undo_turn(&turn, &actor)
        .unwrap();
    assert_eq!(
        d.docs.target("app.vellum").unwrap().snapshot().unwrap(),
        original
    );
}
struct Files {
    reads: Rc<RefCell<Vec<PathBuf>>>,
    allow_image: bool,
}
impl Outside for Files {
    fn read_file(&self, _: &Origin, path: &Path, _: u64) -> Result<Vec<u8>, ToolError> {
        self.reads.borrow_mut().push(path.into());
        if path == Path::new("/repo/test.pen") {
            Ok(fixture().into_bytes())
        } else if self.allow_image && path == Path::new("/repo/images/test.png") {
            vellum_image_ops::encode_png(1, 1, &[10, 20, 30, 255])
                .map_err(|_| ToolError::new(ErrorKind::Document, "fixture"))
        } else {
            Err(ToolError::new(ErrorKind::Refused, "denied"))
        }
    }
    fn network(&self, _: &Origin, _: &str) -> Result<(), ToolError> {
        panic!(".pen import must never fetch remote resources")
    }
}
#[test]
fn source_and_each_companion_image_use_the_broker_and_missing_assets_are_reported() {
    for allow_image in [false, true] {
        let mut d = common::Daemon::new(&document(), &["app.vellum"]);
        let mut s = Session::default();
        run(
            &mut d,
            &mut s,
            "open_document",
            json!({"path":"app.vellum"}),
        )
        .unwrap();
        let reads = Rc::new(RefCell::new(vec![]));
        d.docs.set_outside(Rc::new(Files {
            reads: reads.clone(),
            allow_image,
        }));
        let r = run(
            &mut d,
            &mut s,
            "import",
            json!({"path":"/repo/test.pen","parent":"page000001"}),
        )
        .unwrap();
        assert_eq!(
            r["assets"].as_array().unwrap().len(),
            usize::from(allow_image)
        );
        assert_eq!(
            *reads.borrow(),
            vec![
                PathBuf::from("/repo/test.pen"),
                PathBuf::from("/repo/images/test.png")
            ]
        );
    }
}

#[test]
fn inline_images_and_implicit_path_bounds_use_existing_asset_and_svg_services() {
    let png = vellum_image_ops::encode_png(1, 1, &[10, 20, 30, 255]).unwrap();
    let source = json!({"version":"2.21","children":[{"id":"image","type":"rectangle","width":20,"height":30,"fill":{"type":"image","url":format!("data:image/png;base64,{}",vellum_core::base64::encode(&png))}},{"id":"path","type":"path","geometry":"M10 20 H40 V60 H10 Z","fill":"#123456"}]});
    let mut d = common::Daemon::new(&document(), &["app.vellum"]);
    let mut s = Session::default();
    run(
        &mut d,
        &mut s,
        "open_document",
        json!({"path":"app.vellum"}),
    )
    .unwrap();
    let r = run(
        &mut d,
        &mut s,
        "import",
        json!({"text":source.to_string(),"parent":"page000001"}),
    )
    .unwrap();
    assert_eq!(r["assets"].as_array().unwrap().len(), 1);
    let doc = d.docs.target("app.vellum").unwrap().snapshot().unwrap();
    let path = serde_json::to_value(&doc.pages[0].children[1]).unwrap();
    assert_eq!(
        path["viewBox"],
        json!({"x":10.0,"y":20.0,"width":30.0,"height":40.0})
    );
    assert_eq!(path["width"], 30.0);
    assert_eq!(path["height"], 40.0);
}

#[test]
fn supported_icons_lower_to_the_same_editable_vectors_as_insert_icon() {
    let source = json!({"version":"2.21","children":[{"id":"icon","type":"icon","library":"lucide","icon":"check","width":24,"height":24,"fill":"#123456"}]});
    let mut d = common::Daemon::new(&document(), &["app.vellum"]);
    let mut s = Session::default();
    run(
        &mut d,
        &mut s,
        "open_document",
        json!({"path":"app.vellum"}),
    )
    .unwrap();
    run(
        &mut d,
        &mut s,
        "import",
        json!({"text":source.to_string(),"parent":"page000001"}),
    )
    .unwrap();
    let doc = d.docs.target("app.vellum").unwrap().snapshot().unwrap();
    let node = serde_json::to_value(&doc.pages[0].children[0]).unwrap();
    assert_eq!(node["type"], "vector");
    assert_eq!(node["stroke"]["paints"][0]["color"], "#123456");
    assert!(!node["path"].as_str().unwrap().is_empty());
}
