//! TEST DATA. The write tools, `execute`, batch errors and step descriptors (guide Prompt 30), against the daemon's
//! documents over fakes. `execute` runs on the real QuickJS sandbox.

mod common;

use common::{Daemon, FakeEditor, design};
use serde_json::{Value, json};
use vellum::daemon::EditorId;
use vellum::tools::{Client, ErrorKind, Registry, Session, Step, ToolCtx, ToolError, Verb};
use vellum_types::{Actor, ComponentId, Document, Node, NodeKind};

const PATH: &str = "design/app.vellum";

struct Rig {
    daemon: Daemon,
    registry: Registry,
    session: Session,
}

impl Rig {
    fn headless() -> Self {
        let mut session = Session::default();
        session.client = Some(Client {
            name: "Claude Code".into(),
            version: "2.0.0".into(),
        });
        let mut rig = Self {
            daemon: Daemon::with_scripts(&design(), &[PATH]),
            registry: Registry::standard(),
            session,
        };
        rig.ok("open_document", json!({"path": PATH}));
        rig.session.take_steps();
        rig
    }

    /// Another client on the same daemon.
    fn other(&self) -> Session {
        let mut session = Session::default();
        session.client = Some(Client {
            name: "Codex".into(),
            version: "1.0.0".into(),
        });
        session
    }

    fn call(&mut self, tool: &str, input: Value) -> Result<Value, ToolError> {
        let mut ctx = ToolCtx::new(&mut self.daemon.docs, &mut self.session);
        self.registry.call(&mut ctx, tool, input)
    }

    fn call_as(
        &mut self,
        session: &mut Session,
        tool: &str,
        input: Value,
    ) -> Result<Value, ToolError> {
        let mut ctx = ToolCtx::new(&mut self.daemon.docs, session);
        self.registry.call(&mut ctx, tool, input)
    }

    fn ok(&mut self, tool: &str, input: Value) -> Value {
        self.call(tool, input)
            .unwrap_or_else(|e| panic!("{tool} failed: {e} ({e:?})"))
    }

    fn doc(&mut self) -> Document {
        self.daemon.docs.target(PATH).unwrap().snapshot().unwrap()
    }

    fn last_step(&self) -> &Step {
        self.session.steps().last().expect("a step was recorded")
    }
}

fn find<'a>(doc: &'a Document, id: &str) -> Option<&'a Node> {
    fn walk<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
        nodes.iter().find_map(|n| {
            if n.id().as_str() == id {
                Some(n)
            } else {
                walk(n.children(), id)
            }
        })
    }
    doc.pages
        .iter()
        .find_map(|p| walk(&p.children, id))
        .or_else(|| {
            doc.components
                .values()
                .find_map(|c| walk(std::slice::from_ref(&c.root), id))
        })
}

fn child_ids(doc: &Document, id: &str) -> Vec<String> {
    find(doc, id)
        .unwrap_or_else(|| panic!("{id} exists"))
        .children()
        .iter()
        .map(|n| n.id().as_str().to_owned())
        .collect()
}

fn created(out: &Value) -> Vec<String> {
    out["created"]
        .as_array()
        .unwrap_or_else(|| panic!("created is a list in {out}"))
        .iter()
        .map(|v| v.as_str().expect("an id").to_owned())
        .collect()
}

/// An editor holds the file; returns the fake so the test can see what it applied.
fn live(rig: &mut Rig) -> FakeEditor {
    let editor = FakeEditor::new(&design());
    rig.daemon
        .docs
        .open_editor(PATH, EditorId("tab-1".into()), editor.link())
        .unwrap();
    editor
}

// ---- the registry ---------------------------------------------------------------------------------------

const WRITE_TOOLS: [&str; 19] = [
    "arrange_nodes",
    "claim_frame",
    "copy_nodes",
    "create_component",
    "create_instance",
    "delete_nodes",
    "execute",
    "insert_nodes",
    "instance_nodes",
    "make_slot",
    "manage_components",
    "manage_pages",
    "move_nodes",
    "release_frame",
    "replace_nodes",
    "replace_properties",
    "report_progress",
    "set_variables",
    "update_nodes",
];

#[test]
fn every_write_tool_is_registered_and_none_claims_to_be_read_only() {
    let registry = Registry::standard();
    for name in WRITE_TOOLS {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("{name} is registered"));
        assert!(!tool.read_only(), "{name} changes something");
    }
    for (name, destructive) in [
        ("insert_nodes", false),
        ("create_instance", false),
        ("claim_frame", false),
        ("report_progress", false),
        ("arrange_nodes", false),
        ("manage_pages", true),
        ("manage_components", true),
        ("instance_nodes", true),
        ("update_nodes", true),
        ("delete_nodes", true),
        ("replace_nodes", true),
        ("replace_properties", true),
        ("execute", true),
    ] {
        assert_eq!(
            registry.get(name).unwrap().destructive(),
            destructive,
            "{name}"
        );
    }
}

// ---- insert_nodes -----------------------------------------------------------------------------------------

#[test]
fn insert_nodes_adds_a_batch_in_one_transaction_and_returns_the_new_ids() {
    let mut rig = Rig::headless();
    let editor = live(&mut rig);
    let out = rig.ok(
        "insert_nodes",
        json!({"parent": "frameFoot1", "nodes": [
            {"type": "rect", "name": "Divider", "width": 390, "height": 1},
            {"type": "frame", "name": "Links", "width": 200, "height": 40, "children": [
                {"type": "rect", "name": "Dot", "width": 4, "height": 4}]}
        ]}),
    );
    let ids = created(&out);
    assert_eq!(ids.len(), 2, "one id per top-level node: {out}");
    assert_eq!(out["nodes"], json!(3), "nested nodes are counted: {out}");
    let doc = editor.doc();
    let children = child_ids(&doc, "frameFoot1");
    assert_eq!(&children[2..], &ids[..], "appended in order");
    assert_eq!(find(&doc, &ids[1]).unwrap().children().len(), 1);
    let actors = &editor.0.borrow().actors;
    assert_eq!(actors.len(), 1, "one transaction, one undo step");
    assert!(
        matches!(&actors[0], Actor::Agent { agent } if Some(agent) == rig.session.agent()),
        "attributed to this client's agent: {actors:?}"
    );
}

#[test]
fn insert_nodes_takes_an_index() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "insert_nodes",
        json!({"parent": "frameFoot1", "index": 0, "nodes": [{"type": "rect", "width": 1, "height": 1}]}),
    );
    assert_eq!(child_ids(&rig.doc(), "frameFoot1")[0], created(&out)[0]);
}

#[test]
fn insert_nodes_reads_the_compact_spec() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "insert_nodes",
        json!({"parent": "pageOther1", "format": "jsx", "spec": r##"
            <frame name="Card" width={320} height="hug" layout="column" gap={8}
                   position={{"type": "absolute", "x": 0, "y": 0}}>
              <text name="Title" width="fill" height="hug" font="Inter" size={20} weight={600}>Today</text>
              <text width="fill" height="hug" font="Inter" size={14}>Three habits left</text>
              <instance componentId="compButton1" name="Go"/>
            </frame>
        "##}),
    );
    let ids = created(&out);
    assert_eq!(ids.len(), 1);
    let doc = rig.doc();
    let Some(Node::Frame(card)) = find(&doc, &ids[0]) else {
        panic!("a frame")
    };
    assert_eq!(card.name, "Card");
    assert_eq!(card.children.len(), 3);
    let Node::Text(title) = &card.children[0] else {
        panic!("a text")
    };
    assert_eq!(title.name, "Title");
    assert_eq!(title.content.len(), 1);
    assert_eq!(title.content[0].text, "Today");
    assert_eq!(title.content[0].font, "Inter");
    assert_eq!(title.content[0].weight, 600);
    let Node::Instance(go) = &card.children[2] else {
        panic!("an instance")
    };
    assert_eq!(go.component_id.as_str(), "compButton1");
    assert_eq!(out["nodes"], json!(4));
}

#[test]
fn a_spec_that_does_not_parse_names_the_line_and_column() {
    let mut rig = Rig::headless();
    let before = rig.doc();
    let error = rig
        .call(
            "insert_nodes",
            json!({"parent": "pageOther1", "format": "jsx", "spec": "<frame width={10}>\n  <rect width=10/>\n</frame>"}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.path(), Some("spec"));
    assert!(error.message().contains("line 2"), "{error}");
    assert!(error.message().contains("column"), "{error}");
    assert_eq!(rig.doc(), before);
}

#[test]
fn the_spec_parsers_are_a_registry() {
    let names: Vec<&str> = vellum::tools::spec_formats().collect();
    assert_eq!(names, ["json", "jsx"]);
    let mut rig = Rig::headless();
    let error = rig
        .call(
            "insert_nodes",
            json!({"parent": "pageOther1", "format": "yaml", "spec": "a: b"}),
        )
        .unwrap_err();
    assert_eq!(error.path(), Some("format"));
    assert!(error.message().contains("json, jsx"), "{error}");
    let out = rig.ok(
        "insert_nodes",
        json!({"parent": "pageOther1", "format": "json", "spec": r#"[{"type": "rect", "width": 3, "height": 3}]"#}),
    );
    assert_eq!(created(&out).len(), 1);
}

// ---- batch errors -----------------------------------------------------------------------------------------

#[test]
fn a_bad_node_rejects_the_whole_batch_with_its_index_path_and_reason() {
    let mut rig = Rig::headless();
    let before = rig.doc();
    let error = rig
        .call(
            "insert_nodes",
            json!({"parent": "frameFoot1", "nodes": [
                {"type": "rect", "width": 10, "height": 10},
                {"type": "rect", "width": 10, "height": 10, "opacity": "half"},
            ]}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.op(), Some(1), "{error:?}");
    assert_eq!(error.path(), Some("nodes[1].opacity"), "{error:?}");
    assert!(!error.message().is_empty());
    assert_eq!(rig.doc(), before, "nothing applied");
}

#[test]
fn an_op_the_document_refuses_is_reported_at_its_input() {
    let mut rig = Rig::headless();
    let before = rig.doc();
    let error = rig
        .call(
            "update_nodes",
            json!({"updates": [
                {"id": "textTitle1", "set": {"name": "Heading"}},
                {"id": "nopeNope01", "set": {"name": "Ghost"}},
            ]}),
        )
        .unwrap_err();
    assert_eq!(error.op(), Some(1));
    assert_eq!(error.path(), Some("updates[1].id"), "{error:?}");
    assert!(error.message().contains("nopeNope01"), "{error}");
    assert_eq!(rig.doc(), before);

    let error = rig
        .call(
            "update_nodes",
            json!({"updates": [{"id": "textTitle1", "set": {"cornerRadius": "round"}}]}),
        )
        .unwrap_err();
    assert_eq!(error.op(), Some(0));
    assert!(
        error.path().unwrap().starts_with("updates[0].set"),
        "{error:?}"
    );

    let error = rig
        .call(
            "insert_nodes",
            json!({"parent": "textTitle1", "nodes": [{"type": "rect", "width": 1, "height": 1}]}),
        )
        .unwrap_err();
    assert_eq!(error.path(), Some("parent"), "{error:?}");
    assert!(error.message().contains("textTitle1"), "{error}");
}

#[test]
fn the_agent_can_fix_the_batch_and_retry() {
    let mut rig = Rig::headless();
    let mut nodes = json!([
        {"type": "rect", "width": 10, "height": 10},
        {"type": "rect", "width": "wide", "height": 10},
    ]);
    let error = rig
        .call(
            "insert_nodes",
            json!({"parent": "frameFoot1", "nodes": nodes.clone()}),
        )
        .unwrap_err();
    let at = error.op().unwrap();
    assert_eq!(error.path(), Some("nodes[1].width"), "{error:?}");
    nodes[at]["width"] = json!(10);
    let out = rig.ok(
        "insert_nodes",
        json!({"parent": "frameFoot1", "nodes": nodes}),
    );
    assert_eq!(created(&out).len(), 2);
}

// ---- the other node tools ---------------------------------------------------------------------------------

#[test]
fn update_nodes_sets_and_resets_properties() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "update_nodes",
        json!({"updates": [
            {"id": "textTitle1", "set": {"name": "Heading", "opacity": 0.5}},
            {"id": "rectCard01", "set": {"cornerRadius": 12}},
        ]}),
    );
    assert_eq!(out["changed"], json!(2));
    let doc = rig.doc();
    assert_eq!(find(&doc, "textTitle1").unwrap().name(), "Heading");
    rig.ok(
        "update_nodes",
        json!({"updates": [{"id": "textTitle1", "set": {"opacity": null}}]}),
    );
    let node = serde_json::to_value(find(&rig.doc(), "textTitle1").unwrap()).unwrap();
    assert!(node.get("opacity").is_none(), "null puts it back: {node}");
}

#[test]
fn copy_nodes_returns_the_copies() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "copy_nodes",
        json!({"copies": [{"id": "textTitle1", "parent": "frameFoot1"}, {"id": "rectCard01", "parent": "frameFoot1", "index": 0}]}),
    );
    let ids = created(&out);
    assert_eq!(ids.len(), 2);
    let children = child_ids(&rig.doc(), "frameFoot1");
    assert_eq!(children[0], ids[1]);
    assert_eq!(children.last(), Some(&ids[0]));
    assert!(!ids.contains(&"textTitle1".to_owned()));
}

#[test]
fn move_nodes_moves_and_delete_nodes_deletes() {
    let mut rig = Rig::headless();
    rig.ok(
        "move_nodes",
        json!({"moves": [{"id": "textBody01", "parent": "frameFoot1", "index": 0}]}),
    );
    let doc = rig.doc();
    assert_eq!(child_ids(&doc, "frameFoot1")[0], "textBody01");
    assert!(!child_ids(&doc, "frameHero1").contains(&"textBody01".to_owned()));

    let out = rig.ok("delete_nodes", json!({"ids": ["textBody01", "noteFoot01"]}));
    assert_eq!(out["removed"], json!(2));
    let doc = rig.doc();
    assert!(find(&doc, "textBody01").is_none());
    assert!(find(&doc, "noteFoot01").is_none());
}

#[test]
fn replace_nodes_puts_a_new_node_in_place() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "replace_nodes",
        json!({"replacements": [{"id": "rectCard01", "node": {"type": "ellipse", "name": "Dot", "width": 40, "height": 40}}]}),
    );
    let ids = created(&out);
    let doc = rig.doc();
    assert!(find(&doc, "rectCard01").is_none());
    let hero = child_ids(&doc, "frameHero1");
    assert_eq!(hero[2], ids[0], "in the same place");
}

#[test]
fn replace_properties_changes_every_matching_value_in_a_scope() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "replace_properties",
        json!({"property": "content.size", "from": 16, "to": 18, "scope": "frameHero1"}),
    );
    assert_eq!(out["changed"], json!(1), "{out}");
    let doc = rig.doc();
    let Some(Node::Text(body)) = find(&doc, "textBody01") else {
        panic!()
    };
    assert_eq!(
        serde_json::to_value(&body.content[0].size)
            .unwrap()
            .as_f64(),
        Some(18.0)
    );

    // A colour becomes a variable, named by its path.
    rig.ok(
        "update_nodes",
        json!({"updates": [{"id": "rectCard01", "set": {"fills": [{"type": "solid", "color": "#111111"}]}}]}),
    );
    let out = rig.ok(
        "replace_properties",
        json!({"property": "fills.color", "from": "#111", "to": "$color/surface"}),
    );
    assert_eq!(out["changed"], json!(1), "{out}");
    let card = serde_json::to_value(find(&rig.doc(), "rectCard01").unwrap()).unwrap();
    assert_eq!(card["fills"][0]["color"], json!({"$var": "varSurface1"}));

    let out = rig.ok(
        "replace_properties",
        json!({"property": "cornerRadius", "from": 99, "to": 4}),
    );
    assert_eq!(out["changed"], json!(0), "nothing matched, nothing changed");
}

#[test]
fn set_variables_adds_by_name_updates_and_refuses_to_remove_one_in_use() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "set_variables",
        json!({"variables": [
            {"name": "space/8", "type": "number", "values": [{"value": 32}]},
            {"name": "space/4", "type": "number", "values": [{"value": 20}]},
        ]}),
    );
    let ids = out["ids"].as_object().unwrap();
    assert_eq!(
        ids["space/4"],
        json!("varSpace004"),
        "an existing name keeps its id"
    );
    let new_id = ids["space/8"].as_str().unwrap().to_owned();
    let doc = rig.doc();
    assert!(doc.variables.keys().any(|id| id.as_str() == new_id));

    let before = rig.doc();
    let error = rig
        .call("set_variables", json!({"remove": ["color/surface"]}))
        .unwrap_err();
    assert!(error.message().contains("still used"), "{error}");
    assert_eq!(error.path(), Some("remove[0]"), "{error:?}");
    assert_eq!(rig.doc(), before);
}

#[test]
fn components_instances_and_slots() {
    let mut rig = Rig::headless();
    let out = rig.ok("create_component", json!({"ids": ["frameFoot1"]}));
    let component = out["components"][0].as_str().unwrap().to_owned();
    let doc = rig.doc();
    assert!(doc.components.keys().any(|c| c.as_str() == component));
    assert!(matches!(find(&doc, "frameFoot1"), Some(Node::Instance(_))));

    let out = rig.ok(
        "create_instance",
        json!({"instances": [{"parent": "pageOther1", "component": "compButton1", "set": {"name": "Buy"}}]}),
    );
    let id = created(&out)[0].clone();
    let Some(Node::Instance(instance)) = find(&rig.doc(), &id).cloned() else {
        panic!("an instance")
    };
    assert_eq!(instance.name, "Buy");
    assert_eq!(
        serde_json::to_value(&instance.width).unwrap().as_f64(),
        Some(120.0),
        "sized like the component"
    );

    let out = rig.ok(
        "insert_nodes",
        json!({"parent": "buttonRoot", "nodes": [{"type": "frame", "name": "Icon", "width": 16, "height": 16}]}),
    );
    let frame = created(&out)[0].clone();
    rig.ok(
        "make_slot",
        json!({"slots": [{"id": frame, "suggested": ["compButton1"]}]}),
    );
    let node = serde_json::to_value(find(&rig.doc(), &frame).unwrap()).unwrap();
    assert!(node.get("slot").is_some(), "{node}");
}

// ---- leases, pauses and attribution -----------------------------------------------------------------------

#[test]
fn a_claimed_frame_holds_against_other_clients_until_released() {
    let mut rig = Rig::headless();
    rig.ok("claim_frame", json!({"frame": "frameHero1"}));
    let state = rig.ok("get_app_state", json!({}));
    assert_eq!(state["leases"][0]["frame"], "frameHero1");
    assert_eq!(
        state["leases"][0]["agent"].as_str(),
        rig.session.agent().map(|a| a.as_str())
    );

    let mut other = rig.other();
    let edit = json!({"updates": [{"id": "textTitle1", "set": {"name": "Mine"}}]});
    let error = rig
        .call_as(&mut other, "update_nodes", edit.clone())
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Refused);
    assert!(error.message().to_lowercase().contains("leased"), "{error}");
    let error = rig
        .call_as(&mut other, "claim_frame", json!({"frame": "frameHero1"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Refused);

    // The holder's own writes go through.
    rig.ok(
        "update_nodes",
        json!({"updates": [{"id": "textTitle1", "set": {"name": "Ours"}}]}),
    );
    rig.ok("release_frame", json!({"frame": "frameHero1"}));
    rig.call_as(&mut other, "update_nodes", edit).unwrap();

    let error = rig
        .call("claim_frame", json!({"frame": "textTitle1"}))
        .unwrap_err();
    assert!(error.message().contains("not a frame"), "{error}");
}

#[test]
fn a_paused_agent_cannot_write() {
    let mut rig = Rig::headless();
    rig.ok("get_app_state", json!({}));
    rig.ok("claim_frame", json!({"frame": "frameFoot1"}));
    let agent = rig.session.agent().unwrap().clone();
    let path = rig.daemon.docs.active().unwrap().to_owned();
    rig.daemon
        .docs
        .headless_guard(&path)
        .unwrap()
        .pause
        .pause(agent);
    let before = rig.doc();
    let error = rig
        .call("delete_nodes", json!({"ids": ["textFoot01"]}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Refused);
    assert!(error.message().to_lowercase().contains("paused"), "{error}");
    let error = rig
        .call(
            "execute",
            json!({"code": "doc.delete('textFoot01'); return 1;"}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Refused, "{error:?}");
    assert_eq!(rig.doc(), before);
}

#[test]
fn each_client_is_its_own_agent_named_by_the_client() {
    let mut rig = Rig::headless();
    let mut other = rig.other();
    rig.ok("get_app_state", json!({}));
    rig.call_as(&mut other, "get_app_state", json!({})).unwrap();
    let (ours, theirs) = (rig.session.agent().cloned(), other.agent().cloned());
    assert!(ours.is_some() && theirs.is_some() && ours != theirs);
    let state = rig.ok("get_app_state", json!({}));
    assert_eq!(
        state["you"]["agent"].as_str(),
        ours.as_ref().map(|a| a.as_str())
    );
    assert_eq!(state["you"]["name"], "Claude Code");
    let names: Vec<&str> = state["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"Claude Code") && names.contains(&"Codex"),
        "{state}"
    );
}

// ---- execute ----------------------------------------------------------------------------------------------

const SCRIPT: &str = r#"
const page = doc.page("Other");
const list = doc.insert(page, { type: "frame", name: "Habits", width: 390, height: 300,
                                position: { type: "absolute", x: 0, y: 0 } });
for (const habit of ["Drink water", "Read 10 pages", "Walk 20 min"]) {
  doc.insert(list, { type: "text", name: habit, width: 200, height: 20,
                     content: [{ text: habit, font: "Inter", size: 14 }] });
}
return { list };
"#;

#[test]
fn execute_runs_a_script_as_one_transaction() {
    let mut rig = Rig::headless();
    let editor = live(&mut rig);
    let out = rig.ok("execute", json!({"code": SCRIPT}));
    assert_eq!(out["committed"], json!(true));
    assert_eq!(out["changes"]["created"], json!(4), "{out}");
    let list = out["value"]["list"].as_str().unwrap().to_owned();
    let doc = editor.doc();
    assert_eq!(child_ids(&doc, &list).len(), 3);
    assert_eq!(editor.0.borrow().actors.len(), 1, "one transaction");
    let step = rig.last_step();
    assert_eq!(step.verb, Verb::RanScript);
    assert_eq!(step.script.as_deref(), Some(SCRIPT));
    assert_eq!(step.counts.created, 4);
}

#[test]
fn execute_validate_only_reports_without_changing_anything() {
    let mut rig = Rig::headless();
    let before = rig.doc();
    let out = rig.ok("execute", json!({"code": SCRIPT, "validate_only": true}));
    assert_eq!(out["committed"], json!(false));
    assert_eq!(out["changes"]["created"], json!(4), "{out}");
    assert_eq!(rig.doc(), before);
}

#[test]
fn a_failing_script_applies_nothing_and_names_the_line() {
    let mut rig = Rig::headless();
    let before = rig.doc();
    let error = rig
        .call(
            "execute",
            json!({"code": "const a = doc.insert('pageOther1', {type: 'rect', width: 1, height: 1});\ndoc.update(a, {opacity: 'x'});\nreturn a;"}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput, "{error:?}");
    assert_eq!(error.path(), Some("code"));
    assert_eq!(error.line(), Some(2), "{error:?}");
    assert_eq!(rig.doc(), before);

    let error = rig
        .call("execute", json!({"code": "return (;"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(
        error.message().to_lowercase().contains("syntax") || error.line().is_some(),
        "{error:?}"
    );
}

#[test]
fn a_script_claim_is_a_real_lease() {
    let mut rig = Rig::headless();
    rig.ok(
        "execute",
        json!({"code": "doc.claim('frameHero1'); doc.update('textTitle1', {name: 'Hi'}); return null;"}),
    );
    let mut other = rig.other();
    let error = rig
        .call_as(&mut other, "claim_frame", json!({"frame": "frameHero1"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Refused, "{error:?}");
}

// ---- steps ------------------------------------------------------------------------------------------------

#[test]
fn every_call_leaves_a_step_for_the_agent_panel() {
    let mut rig = Rig::headless();
    rig.ok("get_nodes", json!({"ids": ["frameHero1"]}));
    assert_eq!(rig.last_step().verb, Verb::Read);
    assert_eq!(rig.last_step().tool, "get_nodes");

    rig.ok(
        "insert_nodes",
        json!({"parent": "frameHero1", "nodes": [
            {"type": "rect", "width": 1, "height": 1}, {"type": "rect", "width": 2, "height": 2}]}),
    );
    let step = rig.last_step().clone();
    assert_eq!(step.verb, Verb::Designed);
    assert_eq!(step.counts.created, 2);
    let target = step.target.expect("a target");
    assert_eq!(target.id, "frameHero1");
    assert_eq!(target.name, "Hero");
    let thumbnail = step.thumbnail.expect("designed steps ask for a thumbnail");
    assert_eq!(thumbnail.node, "frameHero1");
    assert!(step.error.is_none());

    rig.ok(
        "set_variables",
        json!({"variables": [{"name": "space/8", "type": "number", "values": [{"value": 32}]}]}),
    );
    assert_eq!(rig.last_step().verb, Verb::UpdatedVariables);
    assert_eq!(rig.last_step().counts.variables, 1);

    rig.ok("claim_frame", json!({"frame": "frameFoot1"}));
    let step = rig.last_step();
    assert_eq!(step.verb, Verb::Claimed);
    assert_eq!(step.target.as_ref().unwrap().name, "Footer");

    let _ = rig.call("delete_nodes", json!({"ids": ["missing001"]}));
    let step = rig.last_step();
    assert_eq!(step.verb, Verb::Deleted);
    assert!(
        step.error
            .as_deref()
            .is_some_and(|e| e.contains("missing001")),
        "{step:?}"
    );
    assert!(step.thumbnail.is_none());

    let json = serde_json::to_value(rig.last_step()).unwrap();
    assert_eq!(json["verb"], "deleted");
    assert_eq!(json["outcome"], "failed");
}

#[test]
fn report_progress_is_a_step_and_changes_nothing() {
    let mut rig = Rig::headless();
    let before = rig.doc();
    rig.ok(
        "report_progress",
        json!({"message": "Built the hero; starting on the footer.", "done": 1, "total": 3}),
    );
    let step = rig.last_step();
    assert_eq!(step.verb, Verb::Progress);
    assert_eq!(
        step.message.as_deref(),
        Some("Built the hero; starting on the footer.")
    );
    assert_eq!(step.progress, Some((1, 3)));
    assert_eq!(rig.doc(), before);
    let error = rig
        .call(
            "report_progress",
            json!({"message": "x", "done": 4, "total": 3}),
        )
        .unwrap_err();
    assert_eq!(error.path(), Some("done"));
}

#[test]
fn the_session_keeps_a_bounded_number_of_steps() {
    let mut rig = Rig::headless();
    for _ in 0..(vellum::tools::STEP_LIMIT + 10) {
        rig.ok("report_progress", json!({"message": "tick"}));
    }
    assert_eq!(rig.session.steps().len(), vellum::tools::STEP_LIMIT);
    let taken = rig.session.take_steps();
    assert_eq!(taken.len(), vellum::tools::STEP_LIMIT);
    assert!(rig.session.steps().is_empty());
}

// ---- scripted provider integration ------------------------------------------------------------------------

#[test]
#[ignore = "model-http: requires loopback sockets; node scripts/model-providers.mjs"]
fn a_scripted_agent_designs_a_screen_on_the_stub_endpoint() {
    use std::sync::Arc;
    use vellum_contracts::{fakes::MemorySecrets, suites::model::block_on};
    use vellum_core::ports::*;
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let completion = |id: &str, name: &str, arguments: Value| {
        StubReply::sse(
            vec![json!({
                "choices":[{"delta":{"tool_calls":[{"index":0,"id":id,"function":{"name":name,"arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}]
            })],
            true,
        )
    };
    let stub = StubEndpoint::start(vec![
        completion("call1", "read_skill", json!({"path":"format"})),
        completion("call2", "insert_nodes", json!({"parent":"pageOther1","nodes":[{"type":"frame","width":"bad","height":240}]})),
        completion("call3", "insert_nodes", json!({"parent":"pageOther1","nodes":[{"type":"frame","name":"Card","width":320,"height":240,"children":[{"type":"rect","width":40,"height":40}]}]})),
    ]).unwrap();
    let provider = vellum_llm_openai_compat::Provider::new(
        ModelConfig {
            id: "scripted".into(),
            endpoint: stub.url("/chat"),
            secret_key: None,
            caps: ModelCaps {
                tools: true,
                ..Default::default()
            },
        },
        Arc::new(MemorySecrets::default()),
        Arc::new(vellum_model_http::HttpTransport::default()),
    )
    .unwrap();
    let mut rig = Rig::headless();
    let before = rig.doc();
    let mut request = ChatRequest::text("fixture-model", "Build a card");
    for turn in 0..3 {
        let mut stream =
            block_on(provider.complete(request.clone(), Cancellation::default())).unwrap();
        let mut calls = vec![];
        while let Some(event) = block_on(stream.next()) {
            if let ModelEvent::ToolCall(call) = event.unwrap() {
                calls.push(call);
            }
        }
        assert_eq!(calls.len(), 1);
        for call in calls {
            let result = rig.call(&call.name, call.arguments.clone());
            if turn == 1 {
                let error = result.as_ref().unwrap_err();
                assert_eq!(error.op(), Some(0));
                assert_eq!(error.path(), Some("nodes[0].width"));
                assert_eq!(rig.doc(), before);
            } else {
                assert!(result.is_ok());
            }
            let failed = result.is_err();
            let result = result.unwrap_or_else(|error| json!({"error":{"message":error.message(),"path":error.path(),"op":error.op()}}));
            request.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ChatPart::ToolCall(call.clone())],
            });
            request.messages.push(ChatMessage {
                role: ChatRole::User,
                content: vec![ChatPart::ToolResult {
                    id: call.id,
                    name: call.name,
                    result,
                    failed,
                }],
            });
        }
    }
    let doc = rig.doc();
    let page = doc
        .pages
        .iter()
        .find(|p| p.id.as_str() == "pageOther1")
        .unwrap();
    assert!(
        page.children
            .iter()
            .any(|n| n.name() == "Card" && n.children().len() == 1)
    );
    assert_eq!(stub.remaining(), 0);
    let requests = stub.requests();
    assert_eq!(requests[2].body["messages"][4]["role"], "tool");
    assert!(
        requests[2].body["messages"][4]["content"]
            .as_str()
            .unwrap()
            .contains("nodes[0].width")
    );
    assert_eq!(rig.session.steps().len(), 3);
}

// ---- pages, component definitions, instances and arrangement (ADR-034) ------------------------------------------

fn page_names(doc: &Document) -> Vec<String> {
    doc.pages.iter().map(|p| p.name.clone()).collect()
}

fn x_of(doc: &Document, id: &str) -> f64 {
    place_of(doc, id).0
}

/// A layer's absolute `(x, y)` as the file holds it: its parent's coordinates.
fn place_of(doc: &Document, id: &str) -> (f64, f64) {
    let node = serde_json::to_value(find(doc, id).expect("exists")).expect("a node");
    (
        node["position"]["x"].as_f64().expect("an absolute x"),
        node["position"]["y"].as_f64().expect("an absolute y"),
    )
}

#[test]
fn manage_pages_adds_renames_moves_and_deletes_pages() {
    let mut rig = Rig::headless();
    let out = rig.ok("manage_pages", json!({"insert": [{"name": "Onboarding"}]}));
    let added = created(&out).remove(0);
    assert_eq!(page_names(&rig.doc()), vec!["Home", "Other", "Onboarding"]);

    rig.ok(
        "manage_pages",
        json!({"update": [{"id": "pageHome01", "name": "Start"}]}),
    );
    rig.ok("manage_pages", json!({"move": [{"id": added, "index": 0}]}));
    assert_eq!(page_names(&rig.doc()), vec!["Onboarding", "Start", "Other"]);

    rig.ok("manage_pages", json!({"delete": ["pageHome01"]}));
    let doc = rig.doc();
    assert_eq!(page_names(&doc), vec!["Onboarding", "Other"]);
    assert!(
        find(&doc, "frameHero1").is_none(),
        "a deleted page takes its layers"
    );
}

#[test]
fn manage_pages_refuses_a_bad_index_and_names_the_item() {
    let mut rig = Rig::headless();
    let err = rig
        .call(
            "manage_pages",
            json!({"insert": [{"name": "Later", "index": 9}]}),
        )
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert_eq!(err.path(), Some("insert[0].index"));
    assert_eq!(page_names(&rig.doc()), vec!["Home", "Other"]);
}

#[test]
fn manage_components_renames_and_keeps_what_it_is_not_given() {
    let mut rig = Rig::headless();
    rig.ok(
        "manage_components",
        json!({"update": [{"id": "compButton1", "name": "Action button"}]}),
    );
    let doc = rig.doc();
    let component = doc
        .components
        .get(&ComponentId::new("compButton1").expect("an id"))
        .expect("the component is still there");
    assert_eq!(component.name, "Action button");
    assert!(
        component.props.contains_key("label"),
        "props that were not given stay as they were"
    );
}

#[test]
fn manage_components_refuses_to_remove_a_definition_an_instance_uses() {
    let mut rig = Rig::headless();
    assert!(
        rig.call("manage_components", json!({"remove": ["compButton1"]}))
            .is_err()
    );
    assert!(
        rig.doc()
            .components
            .contains_key(&ComponentId::new("compButton1").expect("an id"))
    );
}

#[test]
fn instance_nodes_detaches_an_instance_then_its_definition_can_go() {
    let mut rig = Rig::headless();
    rig.ok("instance_nodes", json!({"detach": ["instBtn001"]}));
    let doc = rig.doc();
    assert_eq!(
        find(&doc, "instBtn001").expect("the layer is kept").kind(),
        NodeKind::Frame
    );

    rig.ok("manage_components", json!({"remove": ["compButton1"]}));
    assert!(rig.doc().components.is_empty());
}

#[test]
fn instance_nodes_makes_a_component_from_a_layer() {
    let mut rig = Rig::headless();
    rig.ok(
        "instance_nodes",
        json!({"toggle": [{"id": "rectCard01", "componentId": "compCard01"}]}),
    );
    assert!(
        rig.doc()
            .components
            .contains_key(&ComponentId::new("compCard01").expect("an id"))
    );
}

#[test]
fn instance_nodes_replaces_a_layer_inside_an_instance_and_leaves_the_component_alone() {
    let mut rig = Rig::headless();
    let root = ComponentId::new("compButton1").expect("an id");
    let before = serde_json::to_value(&rig.doc().components[&root].root).expect("a node");
    rig.ok(
        "instance_nodes",
        json!({"replace": [{
            "instance": "instBtn001",
            "path": "buttonRoot/buttonText",
            "node": {"type": "text", "name": "Label", "width": "hug", "height": "hug", "content": [{"text": "Go", "font": "Inter", "size": 14}]}
        }]}),
    );
    let after = serde_json::to_value(&rig.doc().components[&root].root).expect("a node");
    assert_eq!(
        before, after,
        "the main component is not changed by an override"
    );
}

#[test]
fn arrange_nodes_groups_and_ungroups_layers() {
    let mut rig = Rig::headless();
    let out = rig.ok(
        "arrange_nodes",
        json!({"group": [{"ids": ["textTitle1", "textBody01"], "name": "Intro"}]}),
    );
    let group = created(&out).remove(0);
    let doc = rig.doc();
    assert_eq!(child_ids(&doc, &group), vec!["textTitle1", "textBody01"]);
    assert!(child_ids(&doc, "frameHero1").contains(&group));
    assert_eq!(find(&doc, &group).expect("the group").name(), "Intro");
    // The group sits at its children's corner, and each child keeps its place on the canvas inside it.
    assert_eq!(place_of(&doc, &group), (20.0, 20.0));
    assert_eq!(place_of(&doc, "textTitle1"), (0.0, 0.0));
    assert_eq!(place_of(&doc, "textBody01"), (0.0, 60.0));

    rig.ok("arrange_nodes", json!({"ungroup": [group.clone()]}));
    let doc = rig.doc();
    assert!(find(&doc, &group).is_none());
    assert!(child_ids(&doc, "frameHero1").contains(&"textTitle1".to_owned()));
}

#[test]
fn arrange_nodes_refuses_layers_from_different_parents_and_names_the_item() {
    let mut rig = Rig::headless();
    let err = rig
        .call(
            "arrange_nodes",
            json!({"group": [{"ids": ["textTitle1", "textFoot01"]}]}),
        )
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert_eq!(err.path(), Some("group[0].ids[1]"));
}

#[test]
fn arrange_nodes_flips_a_layer_and_aligns_layers_to_an_edge() {
    let mut rig = Rig::headless();
    let card =
        serde_json::to_value(find(&rig.doc(), "rectCard01").expect("exists")).expect("a node");
    rig.ok(
        "arrange_nodes",
        json!({"flip": [{"id": "rectCard01", "axis": "horizontal"}]}),
    );
    let flipped =
        serde_json::to_value(find(&rig.doc(), "rectCard01").expect("exists")).expect("a node");
    assert_ne!(flipped, card);
    rig.ok(
        "arrange_nodes",
        json!({"flip": [{"id": "rectCard01", "axis": "horizontal"}]}),
    );
    let twice =
        serde_json::to_value(find(&rig.doc(), "rectCard01").expect("exists")).expect("a node");
    assert_eq!(twice, card, "two flips are no change");

    rig.ok(
        "arrange_nodes",
        json!({"align": [{"ids": ["rectCard01", "textBody01"], "axis": "horizontal", "alignment": "end"}]}),
    );
    let doc = rig.doc();
    assert_eq!(
        x_of(&doc, "rectCard01"),
        60.0,
        "the card's right edge meets the body's (220)"
    );
    assert_eq!(x_of(&doc, "textBody01"), 20.0);
}

#[test]
fn arrange_nodes_scales_a_layer_about_its_anchor() {
    let mut rig = Rig::headless();
    rig.ok(
        "arrange_nodes",
        json!({"scale": [{"id": "rectCard01", "factor": 2, "anchor": {"x": 0, "y": 0}}]}),
    );
    let doc = rig.doc();
    let node = serde_json::to_value(find(&doc, "rectCard01").expect("exists")).expect("a node");
    assert_eq!(node["width"].as_f64(), Some(320.0));
    assert_eq!(node["height"].as_f64(), Some(160.0));
    assert_eq!(
        place_of(&doc, "rectCard01"),
        (20.0, 120.0),
        "the anchor stays put"
    );
}

#[test]
fn manage_components_reads_props_in_the_file_format_and_names_a_bad_one() {
    let mut rig = Rig::headless();
    let err = rig
        .call(
            "manage_components",
            json!({"update": [{"id": "compButton1", "props": {"label": {"type": "nonsense"}}}]}),
        )
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
    assert_eq!(err.path(), Some("update[0].props"));
}

#[test]
fn no_tool_schema_inlines_the_file_format() {
    // The file format is read with read_skill. A schema that inlines it runs to megabytes; the largest today is a
    // few kilobytes.
    let registry = Registry::standard();
    for tool in registry.tools() {
        let size = serde_json::to_string(&tool.input_schema())
            .expect("a schema")
            .len();
        assert!(
            size < 16_384,
            "{}'s input schema is {size} bytes",
            tool.name()
        );
    }
}
