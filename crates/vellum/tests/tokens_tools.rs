//! TEST DATA. `get_spec`, `export_tokens` and `import_tokens` (guide Prompt 40) against the daemon's documents over
//! fakes: what they return, what they change, and what they refuse.

mod common;

use common::{Daemon, design};
use serde_json::{Value, json};
use vellum::tools::{Client, ErrorKind, Registry, Session, ToolCtx, ToolError, Verb};
use vellum_types::Document;

const PATH: &str = "design/app.vellum";

struct Rig {
    daemon: Daemon,
    registry: Registry,
    session: Session,
}

impl Rig {
    fn new() -> Self {
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

    fn call(&mut self, tool: &str, input: Value) -> Result<Value, ToolError> {
        let mut ctx = ToolCtx::new(&mut self.daemon.docs, &mut self.session);
        self.registry.call(&mut ctx, tool, input)
    }

    fn ok(&mut self, tool: &str, input: Value) -> Value {
        self.call(tool, input)
            .unwrap_or_else(|e| panic!("{tool} failed: {e} ({e:?})"))
    }

    fn doc(&mut self) -> Document {
        self.daemon.docs.target(PATH).unwrap().snapshot().unwrap()
    }

    fn export(&mut self, format: &str) -> String {
        self.ok("export_tokens", json!({"format": format}))["text"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

const CSS: &str = ":root {
  --color-card: var(--color-surface);
  --color-surface: #FFFFFF;
  --space-4: 16px;
}

.dark {
  --color-surface: #111111;
}
";

#[test]
fn the_three_tools_are_registered_with_the_right_flags() {
    let registry = Registry::standard();
    let flags = |name: &str| {
        let tool = registry.get(name).unwrap_or_else(|| panic!("{name}"));
        (
            tool.read_only(),
            tool.destructive(),
            tool.version(),
            tool.verb(),
        )
    };
    assert_eq!(flags("get_spec"), (true, false, 1, Verb::Read));
    assert_eq!(flags("export_tokens"), (true, false, 1, Verb::Read));
    assert_eq!(
        flags("import_tokens"),
        (false, true, 1, Verb::UpdatedVariables)
    );
}

#[test]
fn export_writes_the_documents_variables_per_theme() {
    let mut rig = Rig::new();
    let out = rig.ok("export_tokens", json!({"format": "css-vars"}));
    assert_eq!(out["text"], CSS);
    assert_eq!(out["format"], "css-vars");
    assert_eq!(out["file"], "tokens.css");
    assert_eq!(out["tokens"], 3);
    assert_eq!(out["warnings"], json!([]));
    assert_eq!(
        out["mapping"],
        json!([
            {"axis": "Mode", "mode": "light", "selector": ":root"},
            {"axis": "Mode", "mode": "dark", "selector": ".dark"},
        ])
    );
    assert_eq!(rig.doc(), design(), "exporting changes nothing");
}

#[test]
fn export_in_the_other_formats() {
    let mut rig = Rig::new();
    let tailwind = rig.export("tailwind-v4");
    assert!(tailwind.starts_with("@theme {\n"), "{tailwind}");
    assert!(tailwind.contains("--spacing-4: 16px;"), "{tailwind}");
    let dtcg: Value = serde_json::from_str(&rig.export("dtcg-json")).unwrap();
    assert_eq!(dtcg["color"]["surface"]["$value"], "#FFFFFF");
    assert_eq!(dtcg["color"]["card"]["$value"], "{color.surface}");
    assert_eq!(dtcg["space"]["4"]["$type"], "dimension");
}

#[test]
fn the_person_edits_the_selectors_and_lengths() {
    let mut rig = Rig::new();
    let out = rig.ok(
        "export_tokens",
        json!({
            "format": "css-vars",
            "mapping": [{"axis": "Mode", "mode": "dark", "selector": "[data-theme='dark']"}],
            "lengths": [],
        }),
    );
    let text = out["text"].as_str().unwrap();
    assert!(text.contains("[data-theme=\"dark\"] {"), "{text}");
    assert!(text.contains("--space-4: 16;"), "no lengths, no px: {text}");
    let error = rig
        .call(
            "export_tokens",
            json!({"format": "css-vars", "mapping": [{"axis": "Mode", "mode": "dark", "selector": ":root"}]}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.path(), Some("mapping"));
    assert!(error.message().contains("default values go"), "{error}");
}

#[test]
fn a_format_that_does_not_exist_is_an_input_error() {
    let mut rig = Rig::new();
    let error = rig
        .call("export_tokens", json!({"format": "swift"}))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn export_says_what_it_could_not_write() {
    let mut rig = Rig::new();
    rig.ok(
        "set_variables",
        json!({"variables": [
            {"name": "size/both", "type": "number", "values": [{"value": 1}, {"value": 2, "when": {"Mode": "dark"}}]},
        ]}),
    );
    let out = rig.ok("export_tokens", json!({"format": "css-vars"}));
    assert_eq!(out["warnings"], json!([]), "a value under one axis is fine");
    // A variable with no value by default is left out, and the warning names it.
    rig.ok(
        "set_variables",
        json!({"variables": [
            {"name": "size/dark-only", "type": "number", "values": [{"value": 3, "when": {"Mode": "dark"}}]},
        ]}),
    );
    let out = rig.ok("export_tokens", json!({"format": "css-vars"}));
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["token"], "size/dark-only");
    assert!(
        warnings[0]["message"]
            .as_str()
            .unwrap()
            .contains("no value")
    );
}

#[test]
fn importing_what_was_exported_changes_nothing() {
    let mut rig = Rig::new();
    let before = rig.doc();
    for format in ["css-vars", "tailwind-v4", "dtcg-json"] {
        let text = rig.export(format);
        let out = rig.ok("import_tokens", json!({"format": format, "text": text}));
        assert_eq!(out["applied"], false, "{format}: nothing to apply");
        assert_eq!(out["diff"]["added"], json!([]), "{format}");
        assert_eq!(out["diff"]["changed"], json!([]), "{format}");
        assert_eq!(out["diff"]["removed"], json!([]), "{format}");
        assert_eq!(out["diff"]["unchanged"], 3, "{format}");
        assert_eq!(out["warnings"], json!([]), "{format}");
    }
    assert_eq!(rig.doc(), before);
}

#[test]
fn import_adds_changes_and_keeps_ids() {
    let mut rig = Rig::new();
    let css = CSS
        .replace("#111111", "#0A0A0A")
        .replace("16px", "20px")
        .replace("}\n\n.dark", "  --radius-md: 8px;\n}\n\n.dark");
    let out = rig.ok("import_tokens", json!({"format": "css-vars", "text": css}));
    assert_eq!(out["applied"], true);
    assert_eq!(out["diff"]["added"], json!(["radius/md"]));
    assert_eq!(
        out["diff"]["changed"],
        json!([
            {"name": "color/surface", "what": ["Mode: dark"]},
            {"name": "space/4", "what": ["value"]},
        ])
    );
    assert_eq!(
        out["ids"]["space/4"], "varSpace004",
        "an existing name keeps its id"
    );
    let doc = rig.doc();
    let surface = serde_json::to_value(
        &doc.variables[&vellum_types::VariableId::new("varSurface1").unwrap()],
    )
    .unwrap();
    assert_eq!(
        surface["values"][1],
        json!({"value": "#0A0A0A", "when": {"Mode": "dark"}})
    );
    assert_eq!(doc.variables.len(), 4);
    let step = rig.session.steps().last().unwrap();
    assert_eq!(step.tool, "import_tokens");
    // The next export is the file that was imported, with its tokens in name order.
    assert_eq!(
        rig.export("css-vars"),
        ":root {\n  --color-card: var(--color-surface);\n  --color-surface: #FFFFFF;\n  --radius-md: 8px;\n  --space-4: 20px;\n}\n\n.dark {\n  --color-surface: #0A0A0A;\n}\n"
    );
}

#[test]
fn a_dry_run_shows_the_diff_and_changes_nothing() {
    let mut rig = Rig::new();
    let css = CSS.replace("16px", "20px");
    let out = rig.ok(
        "import_tokens",
        json!({"format": "css-vars", "text": css, "dry_run": true}),
    );
    assert_eq!(out["applied"], false);
    assert_eq!(
        out["diff"]["changed"],
        json!([{"name": "space/4", "what": ["value"]}])
    );
    assert_eq!(rig.doc(), design());
}

#[test]
fn a_mode_the_document_lacks_is_added_with_the_mapping_the_person_gives() {
    let mut rig = Rig::new();
    let css = format!("{CSS}\n[data-density=\"compact\"] {{\n  --space-4: 12px;\n}}\n");
    let mapping = json!([
        {"axis": "Density", "mode": "comfortable", "selector": ":root"},
        {"axis": "Density", "mode": "compact", "selector": "[data-density=\"compact\"]"},
    ]);
    let out = rig.ok(
        "import_tokens",
        json!({"format": "css-vars", "text": css, "mapping": mapping}),
    );
    assert_eq!(out["applied"], true);
    assert_eq!(
        out["themes"],
        json!({"Density": ["comfortable", "compact"], "Mode": ["light", "dark"]})
    );
    let doc = rig.doc();
    assert_eq!(doc.themes["Density"], ["comfortable", "compact"]);
    assert_eq!(
        doc.themes["Mode"],
        ["light", "dark"],
        "the existing axis is untouched"
    );
    let space = serde_json::to_value(
        &doc.variables[&vellum_types::VariableId::new("varSpace004").unwrap()],
    )
    .unwrap();
    assert_eq!(space["values"][1]["value"].as_f64(), Some(12.0));
    assert_eq!(space["values"][1]["when"], json!({"Density": "compact"}));
}

#[test]
fn what_the_file_lacks_is_removed_only_when_asked() {
    let mut rig = Rig::new();
    let css = ":root { --color-surface: #FFFFFF; }\n.dark { --color-surface: #111111; }\n";
    let out = rig.ok("import_tokens", json!({"format": "css-vars", "text": css}));
    assert_eq!(out["diff"]["removed"], json!(["color/card", "space/4"]));
    assert_eq!(rig.doc().variables.len(), 3, "listed, not removed");
    let out = rig.ok(
        "import_tokens",
        json!({"format": "css-vars", "text": css, "remove_missing": true}),
    );
    assert_eq!(out["applied"], true);
    assert_eq!(rig.doc().variables.len(), 1);
}

#[test]
fn removing_a_variable_that_is_in_use_refuses_the_whole_import() {
    let mut rig = Rig::new();
    let css = ":root { --space-4: 99px; }\n";
    let before = rig.doc();
    let error = rig
        .call(
            "import_tokens",
            json!({"format": "css-vars", "text": css, "remove_missing": true}),
        )
        .unwrap_err();
    assert!(error.message().contains("still used"), "{error}");
    assert_eq!(
        rig.doc(),
        before,
        "the change to space/4 was not made either"
    );
}

#[test]
fn a_file_that_cannot_be_read_is_an_input_error_with_its_line() {
    let mut rig = Rig::new();
    let error = rig
        .call(
            "import_tokens",
            json!({"format": "css-vars", "text": ":root {\n  --a: 1;\n"}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.path(), Some("text"));
    assert_eq!(error.line(), Some(1));
    let error = rig
        .call(
            "import_tokens",
            json!({"format": "dtcg-json", "text": "{\"a\": "}),
        )
        .unwrap_err();
    assert_eq!(error.path(), Some("text"));
    assert!(error.line().is_some());
}

#[test]
fn a_set_that_cannot_be_variables_names_the_token() {
    let mut rig = Rig::new();
    let css = ":root { --a: #111111; --b: #222222; --c: var(--a); }\n.dark { --c: 12px; }\n";
    let out = rig.ok("import_tokens", json!({"format": "css-vars", "text": css}));
    let warnings = out["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w["token"] == "c" && w["message"].as_str().unwrap().contains("not a color"))
    );
    // A mismatch the format could not see: a number alias to a color.
    let dtcg = json!({
        "a": {"$type": "color", "$value": "#111111"},
        "b": {"$type": "number", "$value": 1,
              "$extensions": {vellum_types::brand::NAME.to_lowercase(): {"modes": {"Mode": {"dark": "{a}"}}}}},
    });
    let out = rig.ok(
        "import_tokens",
        json!({"format": "dtcg-json", "text": dtcg.to_string(), "dry_run": true}),
    );
    assert!(
        out["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["token"] == "b")
    );
}

#[test]
fn unknown_selectors_are_warnings() {
    let mut rig = Rig::new();
    let css = format!("{CSS}\n.sepia {{ --space-4: 1px; }}\n");
    let out = rig.ok(
        "import_tokens",
        json!({"format": "css-vars", "text": css, "dry_run": true}),
    );
    let warnings = out["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]["message"].as_str().unwrap().contains(".sepia"));
}

// ---- get_spec -------------------------------------------------------------------------------------------

#[test]
fn get_spec_describes_a_frame_with_its_tokens_and_components() {
    let mut rig = Rig::new();
    let out = rig.ok("get_spec", json!({"frame": "frameHero1"}));
    assert_eq!(out["document"], PATH);
    assert_eq!(out["format"], "layout-spec-v1");
    assert_eq!(
        out["frame"],
        json!({"id": "frameHero1", "name": "Hero", "page": "pageHome01"})
    );
    assert_eq!(out["root"]["notes"], "Keep the hero short.");
    let ids: Vec<_> = out["root"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "textTitle1",
            "textBody01",
            "rectCard01",
            "instBtn001",
            "ctxHero001"
        ]
    );
    assert_eq!(
        out["root"]["children"][2]["fills"][0]["color"],
        json!({"$token": "color/surface"})
    );
    let tokens = out["tokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0]["modes"], json!({"Mode: dark": "#111111"}));
    assert_eq!(out["components"][0]["name"], "Button");
    assert_eq!(out["components"][0]["instances"], 1);
    assert_eq!(out["root"]["children"][3]["component"], "Button");
    assert!(!out.to_string().contains("$var"));
}

#[test]
fn get_spec_without_a_stack_lists_the_stacks_and_with_one_includes_its_profile() {
    let mut rig = Rig::new();
    let out = rig.ok("get_spec", json!({"frame": "frameHero1"}));
    assert_eq!(out["stacks"], json!(["shadcn", "react-native", "swiftui"]));
    assert!(out.get("stack").is_none());
    let out = rig.ok(
        "get_spec",
        json!({"frame": "frameHero1", "stack": "react-native"}),
    );
    assert_eq!(out["stack"]["id"], "react-native");
    assert_eq!(out["stack"]["layout"]["row"], "flexDirection: 'row'");
    assert!(out.get("stacks").is_none());
    assert_eq!(
        out["format"], "layout-spec-v1",
        "the spec itself does not change with the stack"
    );
    let error = rig
        .call(
            "get_spec",
            json!({"frame": "frameHero1", "stack": "flutter"}),
        )
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(error.path(), Some("stack"));
    assert!(
        error.message().contains("shadcn, react-native, swiftui"),
        "{error}"
    );
}

#[test]
fn get_spec_for_a_layer_that_is_not_there_is_not_found() {
    let mut rig = Rig::new();
    let error = rig.call("get_spec", json!({"frame": "nope"})).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(error.message().contains("nope"), "{error}");
}

#[test]
fn a_registry_can_be_given_other_stacks() {
    use vellum_core::spec::StackProfiles;
    let mut custom = StackProfiles::builtin().get("shadcn").unwrap().clone();
    custom.id = "flutter".into();
    let mut rig = Rig::new();
    rig.registry = Registry::with_stacks(StackProfiles::builtin().with(custom));
    let out = rig.ok(
        "get_spec",
        json!({"frame": "frameHero1", "stack": "flutter"}),
    );
    assert_eq!(out["stack"]["id"], "flutter");
    assert_eq!(out["stacks"], json!(null));
}
