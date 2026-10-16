//! TEST DATA. The tool registry and the read tools (guide Prompt 29), against the daemon's documents over fakes.
//! The scene-backed tests use the real layout, text and Skia adapters.

mod common;

use common::{Daemon, FakeEditor, design};
use serde_json::{Value, json};
use vellum::daemon::{EditorId, EditorLink, EditorView, LeaseInfo, Paused, Viewport};
use vellum::tools::{ErrorKind, Registry, Session, ToolCtx, ToolError};
use vellum_core::ops::{Applied, Transaction};
use vellum_core::ports::{ChangeSet, FileSystem, ScriptValue, TargetError};
use vellum_types::{AgentId, Document, NodeId};

const PATH: &str = "design/app.vellum";

struct Rig {
    daemon: Daemon,
    registry: Registry,
    session: Session,
}

impl Rig {
    fn headless() -> Self {
        Self::over(Daemon::new(&design(), &[PATH]))
    }

    fn scene() -> Self {
        Self::over(Daemon::with_scene(&design(), &[PATH]))
    }

    fn over(daemon: Daemon) -> Self {
        Self {
            daemon,
            registry: Registry::standard(),
            session: Session::default(),
        }
    }

    fn call(&mut self, tool: &str, input: Value) -> Result<Value, ToolError> {
        let mut ctx = ToolCtx::new(&mut self.daemon.docs, &mut self.session);
        self.registry.call(&mut ctx, tool, input)
    }

    fn ok(&mut self, tool: &str, input: Value) -> Value {
        self.call(tool, input)
            .unwrap_or_else(|e| panic!("{tool} failed: {e}"))
    }
}

fn ids(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is a list in {value}"))
        .iter()
        .map(|n| n["id"].as_str().expect("an id").to_owned())
        .collect()
}

// ---- the registry ---------------------------------------------------------------------------------------

const READ_TOOLS: [&str; 11] = [
    "find_properties",
    "find_space",
    "get_app_state",
    "get_nodes",
    "get_variables",
    "list_components",
    "list_fonts",
    "open_document",
    "query_nodes",
    "screenshot",
    "snapshot_layout",
];

#[test]
fn registry_lists_every_read_tool_once_with_a_version_and_an_object_schema() {
    let registry = Registry::standard();
    let mut names: Vec<_> = registry.tools().map(|t| t.name()).collect();
    names.sort_unstable();
    for tool in READ_TOOLS {
        assert!(names.contains(&tool), "{tool} is registered");
    }
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "no tool name appears twice");
    for tool in registry.tools() {
        assert!(tool.version() >= 1, "{} has a version", tool.name());
        assert!(
            !tool.description().is_empty() && !tool.description().contains('!'),
            "{} has a plain description",
            tool.name()
        );
        let schema = serde_json::to_value(tool.input_schema()).unwrap();
        assert_eq!(schema["type"], "object", "{} takes an object", tool.name());
        assert_eq!(
            schema["additionalProperties"],
            json!(false),
            "{} refuses fields it does not know",
            tool.name()
        );
    }
}

#[test]
fn read_tools_are_marked_read_only() {
    let registry = Registry::standard();
    for tool in READ_TOOLS {
        // open_document can make a file, so it does not claim to be read-only.
        let only_reads = tool != "open_document";
        assert_eq!(
            registry.get(tool).unwrap().read_only(),
            only_reads,
            "{tool}"
        );
    }
}

#[test]
fn an_unknown_tool_is_not_found_and_names_the_ones_that_exist() {
    let mut rig = Rig::headless();
    let error = rig.call("insert_node", json!({})).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(error.message().contains("get_nodes"), "{error}");
}

#[test]
fn bad_input_names_the_field_and_says_what_is_wrong() {
    let mut rig = Rig::headless();
    let error = rig
        .call("get_nodes", json!({"document": PATH, "ids": "frameHero1"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.path(), Some("ids"));
    let error = rig
        .call(
            "get_nodes",
            json!({"document": PATH, "ids": [], "depht": 1}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(error.message().contains("depht"), "{error}");
}

// ---- open_document and get_app_state ----------------------------------------------------------------------

#[test]
fn open_document_opens_headless_and_lists_the_pages() {
    let mut rig = Rig::headless();
    let out = rig.ok("open_document", json!({"path": PATH}));
    assert_eq!(out["path"], PATH);
    assert_eq!(out["mode"], "headless");
    assert_eq!(out["name"], "TEST DATA: tools");
    assert_eq!(out["created"], false);
    assert_eq!(ids(&out, "pages"), ["pageHome01", "pageOther1"]);
    assert_eq!(out["pages"][0]["name"], "Home");
    assert_eq!(out["pages"][0]["nodes"], 9);
}

#[test]
fn open_document_creates_a_file_only_when_asked_and_never_over_an_existing_one() {
    let mut rig = Rig::headless();
    let error = rig
        .call("open_document", json!({"path": "design/new.vellum"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(error.message().contains("create"), "{error}");

    let out = rig.ok(
        "open_document",
        json!({"path": "design/new.vellum", "create": true, "name": "Pricing"}),
    );
    assert_eq!(out["created"], true);
    assert_eq!(out["name"], "Pricing");
    assert!(rig.daemon.fs.exists("design/new.vellum").unwrap());

    let again = rig.ok(
        "open_document",
        json!({"path": "design/new.vellum", "create": true}),
    );
    assert_eq!(
        again["created"], false,
        "an existing file is opened, not replaced"
    );
    assert_eq!(again["name"], "Pricing");
}

#[test]
fn only_vellum_documents_can_be_named() {
    let mut rig = Rig::headless();
    for path in ["design/notes.txt", "design", "design/app.vellum.bak"] {
        let error = rig
            .call("open_document", json!({"path": path}))
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{path}");
        assert!(error.message().contains(".vellum"), "{error}");
    }
    let error = rig
        .call(
            "get_nodes",
            json!({"document": "design/notes.txt", "ids": ["a"]}),
        )
        .unwrap_err();
    assert_eq!(error.path(), Some("document"));
}

#[test]
fn a_call_without_a_document_uses_the_one_just_opened() {
    let mut rig = Rig::headless();
    let error = rig
        .call("get_nodes", json!({"ids": ["frameHero1"]}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(error.message().contains("open_document"), "{error}");
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("get_nodes", json!({"ids": ["frameHero1"]}));
    assert_eq!(out["nodes"][0]["name"], "Hero");
}

#[test]
fn get_app_state_reports_open_documents_the_active_one_and_notes_in_scope() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let state = rig.ok("get_app_state", json!({}));
    assert_eq!(state["active"], PATH);
    assert_eq!(state["documents"][0]["path"], PATH);
    assert_eq!(state["documents"][0]["mode"], "headless");
    assert_eq!(state["documents"][0]["dirty"], false);
    assert_eq!(state["selection"], json!([]));
    assert_eq!(state["viewport"], Value::Null);
    assert_eq!(
        state["paused"],
        json!({"all": false, "agents": [], "stopped": []})
    );
    assert_eq!(state["leases"], json!([]));
    // No selection and no screen: the scope is the first page.
    assert_eq!(state["scope"]["page"], "pageHome01");
    assert_eq!(state["notes"][0]["node"], "frameHero1");
    assert_eq!(state["notes"][0]["text"], "Keep the hero short.");
    assert_eq!(state["context"][0]["node"], "ctxHero001");
    assert_eq!(state["context"][0]["appliesTo"], "frameHero1");
    assert_eq!(
        state["context"][0]["text"],
        "Write like a friend, not a brochure."
    );
}

#[test]
fn get_app_state_before_anything_is_open_says_so_instead_of_failing() {
    let mut rig = Rig::headless();
    let state = rig.ok("get_app_state", json!({}));
    assert_eq!(state["documents"], json!([]));
    assert_eq!(state["active"], Value::Null);
}

#[test]
fn get_app_state_reads_leases_and_pauses_from_the_headless_guard() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let agent = AgentId::new("01JA0000000000000000000A11").unwrap();
    let now = rig.daemon.docs.now();
    {
        let guard = rig.daemon.docs.headless_guard(PATH).unwrap();
        guard
            .leases
            .claim(
                &NodeId::new("frameHero1").unwrap(),
                agent.clone(),
                &guard.blocked,
                now,
            )
            .unwrap();
        guard
            .pause
            .pause(AgentId::new("01JA0000000000000000000B0B").unwrap());
    }
    let state = rig.ok("get_app_state", json!({}));
    assert_eq!(state["leases"][0]["frame"], "frameHero1");
    assert_eq!(state["leases"][0]["agent"], agent.as_str());
    assert_eq!(state["leases"][0]["expiresAt"], now.0 + 60_000);
    assert_eq!(state["paused"]["agents"][0], "01JA0000000000000000000B0B");
}

/// An editor that reports a selection, a screen and a lease, and draws a real picture.
struct Screen {
    inner: FakeEditor,
    view: EditorView,
}

impl EditorLink for Screen {
    fn snapshot(&mut self) -> Result<Document, TargetError> {
        self.inner.snapshot()
    }
    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError> {
        self.inner.transact(tx)
    }
    fn render(&mut self, target: &str, scale: f64) -> Result<ScriptValue, TargetError> {
        Ok(ScriptValue::Map(
            [
                ("mime".to_owned(), ScriptValue::Text("image/png".into())),
                (
                    "base64".to_owned(),
                    ScriptValue::Text(format!("editor:{target}:{scale}")),
                ),
                ("width".to_owned(), ScriptValue::Number(10.0)),
                ("height".to_owned(), ScriptValue::Number(10.0)),
            ]
            .into_iter()
            .collect(),
        ))
    }
    fn save(&mut self) -> Result<Document, TargetError> {
        self.inner.save()
    }
    fn is_dirty(&mut self) -> Result<bool, TargetError> {
        self.inner.is_dirty()
    }
    fn view(&mut self) -> Result<EditorView, TargetError> {
        Ok(self.view.clone())
    }
    fn follow(&mut self, changes: &ChangeSet, document: &Document) -> Result<(), TargetError> {
        self.inner.follow(changes, document)
    }
    fn promote(&mut self) -> Result<(), TargetError> {
        self.inner.promote()
    }
}

fn live_rig() -> Rig {
    let mut rig = Rig::headless();
    let editor = FakeEditor::new(&design());
    let view = EditorView {
        selection: vec![NodeId::new("textFoot01").unwrap()],
        page: Some(vellum_types::PageId::new("pageHome01").unwrap()),
        viewport: Some(Viewport {
            x: 0.0,
            y: 0.0,
            width: 1200.0,
            height: 800.0,
            zoom: 0.5,
        }),
        leases: vec![LeaseInfo {
            frame: NodeId::new("frameHero1").unwrap(),
            agent: AgentId::new("01JA0000000000000000000A11").unwrap(),
            expires_at: 99,
        }],
        paused: Paused {
            all: false,
            agents: vec![],
            stopped: vec![],
        },
    };
    rig.daemon
        .docs
        .open_editor(
            PATH,
            EditorId("tab-1".into()),
            Box::new(Screen {
                inner: editor,
                view,
            }),
        )
        .unwrap();
    rig
}

#[test]
fn get_app_state_for_a_live_document_reports_what_the_editor_shows() {
    let mut rig = live_rig();
    let state = rig.ok("get_app_state", json!({}));
    assert_eq!(state["documents"][0]["mode"], "live");
    assert_eq!(state["documents"][0]["editor"], "tab-1");
    assert_eq!(state["selection"], json!(["textFoot01"]));
    assert_eq!(state["viewport"]["zoom"], 0.5);
    assert_eq!(state["leases"][0]["frame"], "frameHero1");
    // The selection decides the scope: the Footer's own notes and context, and the page-level ones that apply.
    assert_eq!(state["scope"]["nodes"], json!(["textFoot01"]));
    assert_eq!(state["notes"], json!([]));
    assert_eq!(state["context"], json!([]));
}

#[test]
fn tools_cannot_tell_live_from_headless_when_reading() {
    let mut live = live_rig();
    let mut headless = Rig::headless();
    headless.ok("open_document", json!({"path": PATH}));
    let input = json!({"ids": ["frameHero1"], "depth": 3});
    assert_eq!(
        live.ok("get_nodes", input.clone()),
        headless.ok("get_nodes", input)
    );
}

#[test]
fn screenshot_of_a_live_document_is_drawn_by_the_editor() {
    let mut rig = live_rig();
    let out = rig.ok("screenshot", json!({"node": "frameHero1", "scale": 2}));
    assert_eq!(out["image"]["mime"], "image/png");
    assert_eq!(out["image"]["base64"], "editor:frameHero1:2");
}

// ---- get_nodes and query_nodes ------------------------------------------------------------------------------

#[test]
fn get_nodes_returns_subtrees_down_to_the_depth_and_counts_what_it_left_out() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let shallow = rig.ok("get_nodes", json!({"ids": ["frameHero1"], "depth": 0}));
    let hero = &shallow["nodes"][0];
    assert_eq!(hero["id"], "frameHero1");
    assert_eq!(hero["type"], "frame");
    assert!(hero.get("children").is_none());
    assert_eq!(hero["childCount"], 5);

    let one = rig.ok("get_nodes", json!({"ids": ["frameHero1"], "depth": 1}));
    let children = one["nodes"][0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 5);
    assert_eq!(children[0]["id"], "textTitle1");
    assert_eq!(children[0]["content"][0]["text"], "Welcome back");
    assert!(one["nodes"][0].get("childCount").is_none());
    assert_eq!(one["truncated"], false);
}

#[test]
fn get_nodes_reports_ids_that_are_not_there_and_finds_nodes_in_components() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("get_nodes", json!({"ids": ["buttonText", "nope000001"]}));
    assert_eq!(out["nodes"][0]["id"], "buttonText");
    assert_eq!(out["nodes"][0]["component"], "compButton1");
    assert!(out["nodes"][0].get("page").is_none());
    assert_eq!(out["missing"], json!(["nope000001"]));
}

#[test]
fn get_nodes_stops_at_the_node_budget_and_says_it_did() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok(
        "get_nodes",
        json!({"ids": ["frameHero1"], "depth": 4, "max_nodes": 3}),
    );
    assert_eq!(out["truncated"], true);
    let children = out["nodes"][0]["children"].as_array().unwrap();
    assert_eq!(children.len(), 5, "every child is listed");
    assert!(
        children.iter().any(|c| c["truncated"] == true),
        "the ones past the budget are stubs"
    );
}

#[test]
fn query_nodes_filters_by_name_type_text_component_and_variable() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let by_type = rig.ok("query_nodes", json!({"type": "text"}));
    // Component trees are searched too: the Button's label is the last one.
    assert_eq!(
        ids(&by_type, "nodes"),
        ["textTitle1", "textBody01", "textFoot01", "buttonText"]
    );
    assert_eq!(by_type["total"], 4);
    assert_eq!(by_type["nodes"][3]["component"], "compButton1");

    let by_name = rig.ok("query_nodes", json!({"name": "hero"}));
    assert_eq!(ids(&by_name, "nodes"), ["frameHero1"]);

    let by_text = rig.ok("query_nodes", json!({"text": "left off"}));
    assert_eq!(ids(&by_text, "nodes"), ["textBody01"]);

    let by_component = rig.ok("query_nodes", json!({"component": "compButton1"}));
    assert_eq!(ids(&by_component, "nodes"), ["instBtn001"]);

    let by_variable = rig.ok("query_nodes", json!({"variable": "varSurface1"}));
    assert_eq!(ids(&by_variable, "nodes"), ["rectCard01"]);
    let by_variable_name = rig.ok("query_nodes", json!({"variable": "color/surface"}));
    assert_eq!(ids(&by_variable_name, "nodes"), ["rectCard01"]);

    let scoped = rig.ok(
        "query_nodes",
        json!({"type": "text", "scope": "frameFoot1"}),
    );
    assert_eq!(ids(&scoped, "nodes"), ["textFoot01"]);
    let first = &scoped["nodes"][0];
    assert_eq!(first["page"], "pageHome01");
    assert_eq!(first["parent"], "frameFoot1");
    assert_eq!(first["name"], "Legal");
}

#[test]
fn query_nodes_limits_its_answer_and_reports_the_total() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("query_nodes", json!({"limit": 2}));
    assert_eq!(out["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(out["total"], 11, "nine on the page and two in the Button");
    assert_eq!(out["truncated"], true);
}

#[test]
fn query_nodes_refuses_an_unknown_node_type_and_lists_the_valid_ones() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let error = rig
        .call("query_nodes", json!({"type": "button"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(error.message().contains("frame"), "{error}");
}

// ---- find_properties, get_variables, list_components, list_fonts ---------------------------------------------

#[test]
fn find_properties_lists_every_distinct_value_with_how_often_and_where() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("find_properties", json!({"property": "content.size"}));
    let values: Vec<_> = out["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["value"].clone())
        .collect();
    // Every size in the document, the Button's 14 included.
    assert_eq!(values, [json!(12.0), json!(14.0), json!(16.0), json!(32.0)]);
    assert_eq!(out["values"][0]["count"], 1);
    assert_eq!(out["values"][0]["nodes"], json!(["textFoot01"]));

    let fills = rig.ok("find_properties", json!({"property": "fills.color"}));
    assert_eq!(fills["values"][0]["value"], json!({"$var": "varSurface1"}));
    assert_eq!(fills["values"][0]["nodes"], json!(["rectCard01"]));

    let scoped = rig.ok(
        "find_properties",
        json!({"property": "content.size", "scope": "frameHero1"}),
    );
    assert_eq!(scoped["values"].as_array().unwrap().len(), 2);
}

#[test]
fn find_properties_on_a_property_nobody_has_returns_an_empty_list() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("find_properties", json!({"property": "cornerRadius"}));
    assert_eq!(out["values"], json!([]));
    assert_eq!(out["nodesWithProperty"], 0);
}

#[test]
fn get_variables_resolves_each_under_the_theme_and_follows_aliases() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let light = rig.ok("get_variables", json!({}));
    assert_eq!(light["themes"], json!({"Mode": ["light", "dark"]}));
    assert_eq!(light["theme"], json!({"Mode": "light"}));
    let surface = light["variables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "varSurface1")
        .unwrap();
    assert_eq!(surface["name"], "color/surface");
    assert_eq!(surface["type"], "color");
    assert_eq!(surface["value"], "#FFFFFF");
    assert_eq!(surface["values"].as_array().unwrap().len(), 2);

    let dark = rig.ok("get_variables", json!({"theme": {"Mode": "dark"}}));
    let card = dark["variables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "varAlias001")
        .unwrap();
    assert_eq!(
        card["value"], "#111111",
        "an alias takes the value of what it names"
    );
    assert_eq!(card["values"][0], json!({"$alias": "varSurface1"}));

    let named = rig.ok("get_variables", json!({"name": "space"}));
    assert_eq!(ids(&named, "variables"), ["varSpace004"]);
    assert_eq!(named["variables"][0]["value"], 16.0);
}

#[test]
fn get_variables_refuses_a_theme_the_document_does_not_have() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let error = rig
        .call("get_variables", json!({"theme": {"Mode": "sepia"}}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(error.message().contains("sepia"), "{error}");
}

#[test]
fn list_components_shows_props_variants_slots_and_how_many_instances_use_each() {
    let mut rig = Rig::headless();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("list_components", json!({}));
    let button = &out["components"][0];
    assert_eq!(button["id"], "compButton1");
    assert_eq!(button["name"], "Button");
    assert_eq!(button["root"], "buttonRoot");
    assert_eq!(
        button["props"],
        json!({"label": {"type": "text", "default": "Press"}})
    );
    assert_eq!(button["variants"], json!({}));
    assert_eq!(button["slots"], json!([]));
    assert_eq!(button["instances"], 1);
    let none = rig.ok("list_components", json!({"name": "card"}));
    assert_eq!(none["components"], json!([]));
}

#[test]
fn list_fonts_shows_the_documents_own_fonts_and_what_the_engine_can_draw() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("list_fonts", json!({}));
    assert_eq!(
        out["custom"],
        json!([{"asset": "fontAsset01", "family": "Geist", "weight": 600, "style": "normal"}])
    );
    let available: Vec<_> = out["available"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert!(available.contains(&"Inter"), "{available:?}");
    assert_eq!(out["used"], json!(["Inter"]));
}

// ---- snapshot_layout, find_space, screenshot ---------------------------------------------------------------------

#[test]
fn snapshot_layout_gives_computed_bounds_for_a_subtree() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("snapshot_layout", json!({"node": "frameHero1"}));
    let hero = out["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "frameHero1")
        .unwrap();
    assert_eq!(
        hero["bounds"],
        json!({"x": 0.0, "y": 0.0, "width": 390.0, "height": 300.0})
    );
    let card = out["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "rectCard01")
        .unwrap();
    assert_eq!(card["parent"], "frameHero1");
    assert_eq!(card["bounds"]["width"], 160.0);
    assert!(out["problems"].is_array());
}

#[test]
fn snapshot_layout_problems_only_returns_just_the_nodes_that_have_one() {
    let mut doc = design();
    // A child that sticks out of its parent.
    doc.pages[0].children[1] = serde_json::from_value(json!({
        "type": "frame", "id": "frameFoot1", "name": "Footer", "position": {"type": "absolute", "x": 0, "y": 400}, "width": 100, "height": 100,
        "clip": true,
        "children": [{"type": "rect", "id": "rectWide01", "name": "Wide", "position": {"type": "absolute", "x": 50, "y": 0}, "width": 200, "height": 20}]
    })).unwrap();
    let mut rig = Rig::over(Daemon::with_scene(&doc, &[PATH]));
    rig.ok("open_document", json!({"path": PATH}));
    let full = rig.ok("snapshot_layout", json!({"node": "pageHome01"}));
    let only = rig.ok(
        "snapshot_layout",
        json!({"node": "pageHome01", "problems_only": true}),
    );
    assert!(only["nodes"].as_array().unwrap().len() < full["nodes"].as_array().unwrap().len());
    let clipped = only["problems"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "clippedChild")
        .expect("a clipped child");
    assert_eq!(clipped["node"], "rectWide01");
    assert_eq!(clipped["parent"], "frameFoot1");
}

#[test]
fn snapshot_layout_without_a_node_uses_the_first_page() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("snapshot_layout", json!({}));
    assert_eq!(out["target"], "pageHome01");
}

#[test]
fn find_space_returns_free_canvas_clear_of_the_existing_frames() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("find_space", json!({"width": 390, "height": 844}));
    assert_eq!(out["page"], "pageHome01");
    let space = &out["rect"];
    assert_eq!(space["width"], 390.0);
    assert_eq!(space["height"], 844.0);
    let (x, y) = (space["x"].as_f64().unwrap(), space["y"].as_f64().unwrap());
    // Hero spans 0..390 x 0..300 and Footer 0..390 x 400..500: the answer overlaps neither.
    let overlaps =
        |top: f64, bottom: f64| x < 390.0 && x + 390.0 > 0.0 && y < bottom && y + 844.0 > top;
    assert!(!overlaps(0.0, 300.0) && !overlaps(400.0, 500.0), "{space}");
}

#[test]
fn find_space_on_an_empty_page_starts_at_the_origin_or_near_the_point() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok(
        "find_space",
        json!({"page": "pageOther1", "width": 100, "height": 100, "near": {"x": 500, "y": 500}}),
    );
    assert_eq!(out["page"], "pageOther1");
    assert!((out["rect"]["x"].as_f64().unwrap() - 500.0).abs() < 100.0);
}

#[test]
fn screenshot_returns_a_png_the_agent_can_see() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let out = rig.ok("screenshot", json!({"node": "rectCard01"}));
    assert_eq!(out["image"]["mime"], "image/png");
    assert!(
        out["image"]["base64"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );
    assert_eq!(out["image"]["width"], 160.0);
    assert_eq!(out["node"], "rectCard01");
}

#[test]
fn screenshot_of_a_frame_with_a_node_the_renderer_cannot_draw_names_that_node() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    // The Hero holds a context node, which has no drawing yet (reports.md, Prompt 29).
    let error = rig
        .call("screenshot", json!({"node": "frameHero1"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Render);
    assert!(
        error.message().contains("ctxHero001") && error.message().contains("context"),
        "{error}"
    );
}

#[test]
fn screenshot_of_something_that_is_not_there_says_which_id() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    let error = rig
        .call("screenshot", json!({"node": "nope000001"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(error.message().contains("nope000001"), "{error}");
}

#[test]
fn screenshot_refuses_a_scale_outside_the_supported_range() {
    let mut rig = Rig::scene();
    rig.ok("open_document", json!({"path": PATH}));
    for scale in [0.0, -1.0, 9.0] {
        let error = rig
            .call("screenshot", json!({"node": "frameHero1", "scale": scale}))
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "scale {scale}");
        assert_eq!(error.path(), Some("scale"));
    }
}

#[test]
fn read_tools_never_change_the_file_or_leave_the_document_dirty() {
    let mut rig = Rig::scene();
    let before = rig.daemon.fs.read(PATH).unwrap();
    rig.ok("open_document", json!({"path": PATH}));
    for (tool, input) in [
        ("get_app_state", json!({})),
        ("get_nodes", json!({"ids": ["frameHero1"]})),
        ("query_nodes", json!({})),
        ("snapshot_layout", json!({})),
        ("find_properties", json!({"property": "content.size"})),
        ("get_variables", json!({})),
        ("list_components", json!({})),
        ("list_fonts", json!({})),
        ("find_space", json!({"width": 10, "height": 10})),
        ("screenshot", json!({"node": "rectCard01"})),
    ] {
        rig.ok(tool, input);
    }
    assert_eq!(rig.daemon.fs.read(PATH).unwrap(), before);
    assert_eq!(
        rig.ok("get_app_state", json!({}))["documents"][0]["dirty"],
        false
    );
}
