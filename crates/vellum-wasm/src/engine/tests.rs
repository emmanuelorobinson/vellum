//! TEST DATA. The editor engine natively: the same code the Emscripten module runs.
use super::*;
use vellum_core::ops::{Op, Transaction};
use vellum_types::{AgentId, DeviceId, Origin};

const DOC: &str = r##"{"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"TEST DATA"},"pages":[{"id":"page000001","name":"TEST DATA","children":[
 {"id":"frame00001","type":"frame","name":"Frame","width":200,"height":100,"position":{"type":"absolute","x":100,"y":50},"fills":[{"type":"solid","color":"#FFFFFF"}],"children":[
   {"id":"rect000001","type":"rect","name":"Rect","width":20,"height":20,"position":{"type":"absolute","x":5,"y":5},"fills":[{"type":"solid","color":"#2878C8"}]},
   {"id":"text000001","type":"text","name":"Label","content":[{"text":"TEST DATA","font":"Inter","size":12}],"width":"hug","height":"hug","position":{"type":"absolute","x":40,"y":5}}
 ]}]}]}"##;

const PERSON: &str = "01JA0000000000000000000A11";

fn options(peer: u64) -> Options {
    Options {
        path: "/designs/test.vellum".into(),
        peer,
        seed: 7,
        read_only: false,
    }
}

fn engine() -> Engine {
    let mut e = Engine::open(DOC.as_bytes(), options(11)).expect("open");
    e.register_font(
        include_bytes!("../../../adapters/text-parley/fonts/InterVariable.ttf").to_vec(),
    )
    .expect("Inter");
    e
}

fn person() -> Value {
    json!({"kind": "person", "device": PERSON})
}

fn insert_rect(id: &str) -> Value {
    json!({
        "origin": {"actor": person()},
        "ops": [{"type": "insert", "to": {"parent": {"type": "node", "id": "frame00001"}, "index": 2},
                 "node": {"id": id, "type": "rect", "name": "New", "width": 10, "height": 10,
                          "position": {"type": "absolute", "x": 150, "y": 60}}}]
    })
}

#[test]
fn opens_a_file_and_answers_scene_questions_for_its_page() {
    let mut e = engine();
    let snapshot = e.snapshot();
    assert_eq!(snapshot["readOnly"], false);
    assert_eq!(snapshot["document"]["pages"][0]["id"], "page000001");
    assert_eq!(e.scene_roots(), json!(["frame00001"]));
    let item = e.scene_get("frame00001/rect000001");
    assert_eq!(
        item["bounds"],
        json!({"x": 105.0, "y": 55.0, "width": 20.0, "height": 20.0})
    );
    assert_eq!(item["parent"], "frame00001");
    assert_eq!(item["node"]["name"], "Rect");
    assert_eq!(item["selectable"], true);
    let frame = e.scene_get("frame00001");
    assert_eq!(frame["children"][0], "frame00001/rect000001");
    assert!(
        frame["node"].get("children").is_none(),
        "children are listed as addresses"
    );
    assert_eq!(
        e.hit_point(Point { x: 110., y: 60. }, true),
        json!("frame00001/rect000001")
    );
    assert_eq!(
        e.hit_point(Point { x: 110., y: 60. }, false),
        json!("frame00001")
    );
    assert_eq!(e.hit_point(Point { x: 0., y: 0. }, true), Value::Null);
    let text = e.scene_get("frame00001/text000001");
    assert!(
        text["bounds"]["width"].as_f64().unwrap() > 0.,
        "text measured with Inter"
    );
    assert_eq!(e.scene_get("nothing"), Value::Null);
}

#[test]
fn coordinates_follow_the_parent_and_the_viewbox() {
    let mut e = engine();
    let local = e.to_parent(&json!("frame00001/rect000001"), Point { x: 110., y: 60. });
    assert_eq!(local, Point { x: 10., y: 10. });
    let place = json!({"parent": {"type": "node", "id": "frame00001"}, "index": 0});
    assert_eq!(
        e.to_parent(&place, Point { x: 110., y: 60. }),
        Point { x: 10., y: 10. }
    );
    let page = json!({"parent": {"type": "page", "id": "page000001"}, "index": 0});
    assert_eq!(
        e.to_parent(&page, Point { x: 110., y: 60. }),
        Point { x: 110., y: 60. }
    );
    assert_eq!(
        e.to_local("frame00001/rect000001", Point { x: 110., y: 60. }),
        Point { x: 5., y: 5. }
    );
    let path = e.path("frame00001/rect000001");
    assert_eq!(path["viewBox"]["width"], 20.0);
    assert_eq!(path["contours"][0]["closed"], true);
    assert_eq!(path["contours"][0]["points"].as_array().unwrap().len(), 4);
    assert_eq!(e.path("frame00001/text000001"), Value::Null);
}

#[test]
fn a_person_edits_undoes_and_redoes_their_own_change() {
    let mut e = engine();
    let before = e.revision();
    let applied = e.apply(&insert_rect("rect000002"));
    assert_eq!(applied["status"], "applied", "{applied}");
    assert!(e.revision() > before);
    assert_eq!(
        e.scene_get("frame00001/rect000002")["bounds"]["x"],
        json!(250.0)
    );
    assert_eq!(e.undo(&person())["status"], "applied");
    assert_eq!(e.scene_get("frame00001/rect000002"), Value::Null);
    assert_eq!(e.redo(&person())["status"], "applied");
    assert_ne!(e.scene_get("frame00001/rect000002"), Value::Null);
    // Someone else's undo has nothing of theirs to take back.
    let other = json!({"kind": "person", "device": "01JA0000000000000000000B0B"});
    assert_eq!(e.undo(&other)["reason"], "unavailable");
    // Invalid requests change nothing.
    assert_eq!(
        e.apply(&insert_rect("rect000002"))["reason"],
        "invalid",
        "duplicate id"
    );
    let agent = json!({"origin": {"actor": {"kind": "agent", "agent": "01JA00000000000000000TEA10"}},
        "ops": insert_rect("rect000003")["ops"]});
    assert_eq!(
        e.apply(&agent)["reason"],
        "invalid",
        "agents do not edit through the editor"
    );
    assert_eq!(
        e.apply(&json!({"origin": {"actor": person()}, "ops": []}))["reason"],
        "invalid"
    );
    assert_eq!(e.next_id().len(), 10);
    assert_ne!(e.next_id(), e.next_id());
}

#[test]
fn read_only_refuses_every_write() {
    let mut e = engine();
    e.set_read_only(true);
    assert_eq!(e.snapshot()["readOnly"], true);
    assert_eq!(e.apply(&insert_rect("rect000002"))["reason"], "read-only");
    assert_eq!(e.undo(&person())["reason"], "read-only");
}

#[test]
fn replicates_the_daemons_engine_both_ways() {
    let genesis = vellum_codec_json::decode(DOC.as_bytes()).unwrap();
    let mut daemon = LoroDocStore::new(&genesis, 1);
    let agent = Actor::Agent {
        agent: AgentId::new("01JA00000000000000000TEA10").unwrap(),
    };
    ops::apply(
        &mut daemon,
        &mut Ids(1),
        Transaction::new(
            Origin::new(agent.clone()),
            vec![Op::Update {
                id: NodeId::new("rect000001").unwrap(),
                patch: serde_json::from_value(json!({"name": "Before the editor"})).unwrap(),
            }],
        ),
    )
    .unwrap();
    let start = daemon.export_updates(&SyncCursor::empty());
    let mut e = Engine::replica(DOC.as_bytes(), &start.bytes, options(12)).expect("replica");
    assert_eq!(
        e.scene_get("frame00001/rect000001")["node"]["name"],
        "Before the editor"
    );
    // The person's edit goes up as their own.
    let seen = daemon.cursor();
    e.apply(&insert_rect("rect000002"));
    let up = SyncPayload {
        adapter: daemon.adapter_id().into(),
        bytes: e.updates_since(&seen.0),
    };
    let person = Actor::Person {
        device: DeviceId::new(PERSON).unwrap(),
    };
    daemon
        .import_updates_as(&up, &person)
        .expect("the person's edit");
    assert_eq!(daemon.snapshot(), e.document());
    // An agent's edit comes down.
    ops::apply(
        &mut daemon,
        &mut Ids(2),
        Transaction::new(
            Origin::new(agent),
            vec![Op::Update {
                id: NodeId::new("rect000001").unwrap(),
                patch: serde_json::from_value(json!({"name": "By an agent"})).unwrap(),
            }],
        ),
    )
    .unwrap();
    let down = daemon.export_updates(&SyncCursor(e.cursor()));
    let revision = e.revision();
    assert!(e.import(&down.bytes).unwrap());
    assert!(e.revision() > revision);
    assert_eq!(
        e.scene_get("frame00001/rect000001")["node"]["name"],
        "By an agent"
    );
    assert!(
        !e.import(&down.bytes).unwrap(),
        "the same updates twice change nothing"
    );
    // The agent's change is not the person's to undo.
    assert_eq!(
        e.undo(&json!({"kind": "person", "device": PERSON}))["status"],
        "applied"
    );
    assert_eq!(e.scene_get("frame00001/rect000002"), Value::Null);
    assert_eq!(
        e.scene_get("frame00001/rect000001")["node"]["name"],
        "By an agent"
    );
    assert!(Engine::replica(DOC.as_bytes(), b"garbage", options(13)).is_err());
}

#[test]
fn draws_the_viewport_and_saves_canonical_bytes() {
    let mut e = engine();
    let frame = e
        .frame(&json!({"width": 400, "height": 300, "dpr": 2, "viewport": {"x": 0, "y": 0, "zoom": 1}}))
        .expect("frame");
    assert!(!frame.list.is_empty());
    assert!(!frame.fonts.is_empty(), "the label draws with Inter");
    assert!(e.frame(&json!({"width": 0})).is_err());
    let bytes = e.encode().unwrap();
    assert_eq!(
        vellum_codec_json::decode(&bytes).unwrap(),
        e.document(),
        "Save writes the engine's document"
    );
    // Canonical: encoding the decoded bytes again changes nothing.
    assert_eq!(
        vellum_codec_json::encode(&vellum_codec_json::decode(&bytes).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn snapping_and_marquee_use_the_scene() {
    let mut e = engine();
    let snapped = e.snap(&json!({
        "box": {"x": 103.5, "y": 200, "width": 10, "height": 10},
        "exclude": [],
        "options": {"search": {"x": 0, "y": 0, "width": 1000, "height": 1000}, "tolerance": 6,
                    "zoom": 1, "edges": true, "centres": false, "equalSpacing": false, "grid": null}
    }));
    assert_eq!(
        snapped["offset"]["x"],
        json!(1.5),
        "the rectangle's left edge is nearest: {snapped}"
    );
    assert_eq!(snapped["guides"][0]["axis"], "x");
    assert_eq!(
        e.hit_rect(
            &json!({"x": 100, "y": 50, "width": 30, "height": 30}),
            true,
            true
        ),
        json!(["frame00001/rect000001"])
    );
}

#[test]
fn one_json_call_reaches_every_facade_method() {
    let mut e = engine();
    assert_eq!(
        e.call(&json!({"call": "roots"})).unwrap(),
        json!(["frame00001"])
    );
    assert_eq!(
        e.call(&json!({"call": "hitPoint", "point": {"x": 110, "y": 60}, "depth": "deepest"}))
            .unwrap(),
        json!("frame00001/rect000001")
    );
    assert_eq!(
        e.call(&json!({"call": "apply", "transaction": insert_rect("rect000002")}))
            .unwrap()["status"],
        "applied"
    );
    assert_eq!(
        e.call(&json!({"call": "toLocal", "address": "frame00001/rect000001", "point": {"x": 110, "y": 60}}))
            .unwrap(),
        json!({"x": 5.0, "y": 5.0})
    );
    e.call(&json!({"call": "setPage", "page": "page000001"}))
        .unwrap();
    assert!(e.call(&json!({"call": "nope"})).is_err());
    assert!(e.call(&json!({"call": "get"})).is_err());
}
