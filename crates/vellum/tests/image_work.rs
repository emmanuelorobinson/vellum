//! TEST DATA. Generated fills, local operations and imports must use attributed undoable Ops.
mod common;
use serde_json::{Value, json};
use std::{path::Path, rc::Rc};
use vellum::{
    permissions::Outside,
    tools::{Registry, Session, ToolCtx, ToolError},
};
use vellum_core::ports::*;
use vellum_types::{Document, Origin};
struct Permit;
impl Outside for Permit {
    fn network(&self, _: &Origin, _: &str) -> Result<(), ToolError> {
        Ok(())
    }
    fn read_file(&self, _: &Origin, _: &Path, _: u64) -> Result<Vec<u8>, ToolError> {
        unreachable!()
    }
}
struct Generator;
impl ImageGenerator for Generator {
    fn provider(&self) -> &str {
        "fixture"
    }
    fn origin(&self) -> &str {
        "https://images.example.com"
    }
    fn generate(&self, _: &GenerateImage) -> Result<ImageBytes, ImageWorkError> {
        Ok(ImageBytes {
            mime: "image/png".into(),
            bytes: vellum_image_ops::encode_png(8, 8, &[40, 180, 90, 255].repeat(64)).unwrap(),
        })
    }
}
fn run(
    d: &mut common::Daemon,
    s: &mut Session,
    name: &str,
    input: Value,
) -> Result<Value, ToolError> {
    Registry::standard().call(&mut ToolCtx::new(&mut d.docs, s), name, input)
}
#[test]
fn generation_and_local_trace_are_one_transaction_each_and_undo_restores_image() {
    let doc:Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},"pages":[{"id":"page000001","name":"Page","children":[{"type":"rect","id":"rect000001","name":"Picture","width":100,"height":60,"fills":[]}]}]})).unwrap();
    let mut d = common::Daemon::new(&doc, &["app.vellum"]);
    let mut s = Session::default();
    d.docs.set_image_generator(Rc::new(Generator));
    d.docs
        .set_image_ops(Rc::new(vellum_image_ops::LocalImageOps::new(Some(
            Box::new(vellum_contracts::fakes::MemorySegmentation),
        ))));
    run(
        &mut d,
        &mut s,
        "open_document",
        json!({"path":"app.vellum"}),
    )
    .unwrap();
    let input = json!({"node":"rect000001","prompt":"TEST DATA leaf","model":"fixture"});
    assert!(run(&mut d, &mut s, "fill_image", input.clone()).is_err());
    d.docs.set_outside(Rc::new(Permit));
    let turn = vellum_types::TurnId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    s.set_turn(Some(turn.clone()));
    run(&mut d, &mut s, "fill_image", input).unwrap();
    let generated = d.docs.target("app.vellum").unwrap().snapshot().unwrap();
    let n = serde_json::to_value(&generated.pages[0].children[0]).unwrap();
    assert_eq!(n["metadata"]["generated"]["provider"], "fixture");
    let background_turn = vellum_types::TurnId::new("01ARZ3NDEKTSV4RRFFQ69G5FAY").unwrap();
    s.set_turn(Some(background_turn.clone()));
    run(
        &mut d,
        &mut s,
        "image_ops",
        json!({"node":"rect000001","action":"remove_background"}),
    )
    .unwrap();
    assert_ne!(
        d.docs.target("app.vellum").unwrap().snapshot().unwrap(),
        generated
    );
    let background_person = vellum_types::Actor::Person {
        device: vellum_types::DeviceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap(),
    };
    d.docs
        .target("app.vellum")
        .unwrap()
        .undo_turn(&background_turn, &background_person)
        .unwrap();
    assert_eq!(
        d.docs.target("app.vellum").unwrap().snapshot().unwrap(),
        generated
    );
    let trace_turn = vellum_types::TurnId::new("01ARZ3NDEKTSV4RRFFQ69G5FAX").unwrap();
    s.set_turn(Some(trace_turn.clone()));
    let trace = run(
        &mut d,
        &mut s,
        "image_ops",
        json!({"node":"rect000001","action":"trace"}),
    )
    .unwrap();
    assert_eq!(trace["node"], "rect000001");
    assert_ne!(
        d.docs.target("app.vellum").unwrap().snapshot().unwrap(),
        generated
    );
    let person = vellum_types::Actor::Person {
        device: vellum_types::DeviceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap(),
    };
    d.docs
        .target("app.vellum")
        .unwrap()
        .undo_turn(&trace_turn, &person)
        .unwrap();
    assert_eq!(
        d.docs.target("app.vellum").unwrap().snapshot().unwrap(),
        generated
    );
    d.docs
        .target("app.vellum")
        .unwrap()
        .undo_turn(&turn, &person)
        .unwrap();
    assert_eq!(
        d.docs.target("app.vellum").unwrap().snapshot().unwrap(),
        doc
    );
}

struct Figma;
impl FigmaSource for Figma {
    fn import_file(
        &self,
        _: &str,
        allow: &dyn Fn(&str) -> Result<(), ImageWorkError>,
    ) -> Result<Imported, ImageWorkError> {
        allow("https://api.figma.com")?;
        let bytes=serde_json::to_vec(&json!({"document":{"type":"DOCUMENT","children":[{"id":"1:1","name":"TEST DATA frame","type":"FRAME","absoluteBoundingBox":{"x":20,"y":30,"width":200,"height":100},"children":[{"type":"TEXT","id":"1:2","characters":"TEST DATA title","absoluteBoundingBox":{"x":24,"y":38,"width":100,"height":20},"style":{"fontFamily":"Inter","fontSize":14,"fontWeight":400}}]}]}})).unwrap();
        vellum_import_figma_api::FigmaImporter
            .import(&ImportInput {
                bytes: &bytes,
                name: "TEST DATA",
            })
            .map_err(|_| ImageWorkError::InvalidOutput)
    }
}
#[test]
fn rest_import_goes_through_ops_and_raw_source_never_enters_step_history() {
    let doc:Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},"pages":[{"id":"page000001","name":"Page"}]})).unwrap();
    let mut d = common::Daemon::new(&doc, &["app.vellum"]);
    let mut s = Session::default();
    d.docs.set_figma(Rc::new(Figma));
    run(
        &mut d,
        &mut s,
        "open_document",
        json!({"path":"app.vellum"}),
    )
    .unwrap();
    assert!(
        run(
            &mut d,
            &mut s,
            "import",
            json!({"figma":"test","parent":"page000001"})
        )
        .is_err()
    );
    d.docs.set_outside(Rc::new(Permit));
    let turn = vellum_types::TurnId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    s.set_turn(Some(turn.clone()));
    let reply = run(
        &mut d,
        &mut s,
        "import",
        json!({"figma":"https://www.figma.com/design/test/TEST_DATA","parent":"page000001"}),
    )
    .unwrap();
    assert_eq!(reply["nodes"], 2);
    assert_eq!(reply["frames"], 1);
    assert_eq!(reply["importer"], "figma-api");
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
        doc
    );
    let secret = "TEST_DATA_INVALID_URL_CANARY_66";
    assert!(run(&mut d,&mut s,"import",json!({"figma":format!("https://user:{secret}@www.figma.com/design/test/TEST_DATA"),"parent":"page000001"})).is_err());
    assert!(!format!("{:?}", s.steps()).contains(secret));
}
#[test]
fn settings_never_store_keys_and_invalid_settings_do_not_persist() {
    let secrets = vellum_contracts::fakes::MemorySecrets::default();
    vellum::asset_settings::save_key(&secrets, "figma", "TEST_DATA_CANARY_66").unwrap();
    assert!(secrets.get("figma_token").unwrap().is_some());
    assert!(vellum::asset_settings::save_key(&secrets, "image", "bad\nkey").is_err());
    let bad = vellum::asset_settings::AssetSettings {
        provider: Some("configured".into()),
        endpoint: Some("https://user:secret@example.com/images".into()),
        model: Some("fixture".into()),
        ..Default::default()
    };
    assert!(bad.validate().is_err());
    assert!(
        serde_json::from_value::<vellum::asset_settings::AssetSettings>(
            json!({"key":"never persisted"})
        )
        .is_err()
    );
}

#[test]
fn native_cli_saves_only_public_settings_and_invalid_key_kind_never_echoes_input() {
    use std::process::{Command, Stdio};
    let root = std::env::temp_dir().join(format!("vellum-image-settings-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let run_dir = root.join("run");
    let binary = env!("CARGO_BIN_EXE_vellum");
    let out = Command::new(binary)
        .args(["--json", "--run-dir"])
        .arg(&run_dir)
        .args([
            "asset-settings",
            "--provider",
            "configured",
            "--endpoint",
            "https://images.example.com/v1/images/generations",
            "--model",
            "fixture",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "native public settings configuration failed"
    );
    let stored: Value =
        serde_json::from_slice(&std::fs::read(root.join("image-settings.json")).unwrap()).unwrap();
    assert_eq!(stored["model"], "fixture");
    assert_eq!(stored.as_object().unwrap().len(), 4);
    let canary = "TEST_DATA_INVALID_KIND_CANARY_66";
    let bad = Command::new(binary)
        .args(["--json", "--run-dir"])
        .arg(&run_dir)
        .args(["asset-key", canary])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(!String::from_utf8_lossy(&bad.stdout).contains(canary));
    assert!(!String::from_utf8_lossy(&bad.stderr).contains(canary));
    std::fs::remove_dir_all(root).unwrap();
}
