//! TEST DATA. Prompt 31: content sources and the tools shared by MCP, CLI and agents.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::{Daemon, design};
use serde_json::{Value, json};
use vellum::content::{BuiltinSkills, BuiltinStyles, FileSkills, FolderStyles, StackedSkills};
use vellum::tools::{ErrorKind, Registry, Session, ToolCtx};
use vellum_contracts::fakes::MemFs;
use vellum_core::ports::{FileSystem, SkillSource, StyleLibrary};

fn call(registry: &Registry, name: &str, input: Value) -> Result<Value, vellum::tools::ToolError> {
    let mut daemon = Daemon::new(&design(), &[]);
    let mut session = Session::default();
    registry.call(
        &mut ToolCtx::new(&mut daemon.docs, &mut session),
        name,
        input,
    )
}

#[test]
fn builtin_skills_include_format_execute_declarations_and_four_workflows() {
    let skills = BuiltinSkills;
    let paths: Vec<_> = skills.list().unwrap().into_iter().map(|m| m.path).collect();
    assert_eq!(paths.len(), 7);
    for path in [
        "format.md",
        "execute.md",
        "vellum-doc.d.ts",
        "workflows/design-a-screen.md",
        "workflows/build-a-design-system.md",
        "workflows/recreate-from-code.md",
        "workflows/make-a-chart-with-a-script.md",
    ] {
        assert!(paths.iter().any(|p| p == path), "{path}");
        assert!(!skills.read(path).unwrap().trim().is_empty());
    }
    assert_eq!(
        skills.read("vellum-doc.d.ts").unwrap(),
        vellum_core::doc_api::declarations()
    );
    assert!(skills.read("missing.md").is_err());
}

#[test]
fn stacking_keeps_builtin_then_guidelines_then_user_folders_and_reads_fresh_content() {
    let fs = Arc::new(MemFs::new());
    fs.seed(
        "systems/minimal/guidelines.md",
        b"# TEST DATA Minimal\n\nUse existing variables.\n",
    );
    fs.seed(
        "skills/voice/SKILL.md",
        b"---\nname: test-voice\ndescription: A test voice.\n---\nUse short labels.\n",
    );
    let skills = StackedSkills::new(vec![
        Rc::new(BuiltinSkills),
        Rc::new(FileSkills::guidelines(fs.clone(), vec!["systems/minimal".into()]).unwrap()),
        Rc::new(FileSkills::folders(fs.clone(), vec!["skills/voice".into()]).unwrap()),
    ]);
    let list = skills.list().unwrap();
    assert_eq!(list[7].path, "systems/minimal/guidelines.md");
    assert_eq!(list[8].name, "test-voice");
    assert_eq!(list[8].description, "A test voice.");
    assert!(skills.read(&list[8].path).unwrap().contains("short labels"));
    fs.seed(
        "skills/voice/SKILL.md",
        b"---\nname: test-voice\ndescription: Changed.\n---\nChanged instructions.\n",
    );
    assert!(
        skills
            .read(&list[8].path)
            .unwrap()
            .contains("Changed instructions")
    );
    fs.remove_file("skills/voice/SKILL.md").unwrap();
    assert!(
        skills.list().is_err(),
        "missing configured folders are explicit"
    );
    assert!(skills.read(&list[8].path).is_err(), "no stale cache");
    assert_eq!(fs.mutations(), 1, "sources only read");
}

#[test]
fn folder_sources_refuse_unregistered_paths_bad_metadata_and_oversized_content() {
    let fs = Arc::new(MemFs::new());
    fs.seed(
        "voice/SKILL.md",
        b"---\nname: voice\ndescription: TEST DATA\n---\nInstructions.\n",
    );
    fs.seed("private.txt", b"must not be returned");
    let source = FileSkills::folders(fs.clone(), vec!["voice".into()]).unwrap();
    for path in [
        "../private.txt",
        "/private.txt",
        "voice/../private.txt",
        "private.txt",
        "voice/other.md",
        "voice\\SKILL.md",
    ] {
        assert!(source.read(path).is_err(), "{path}");
    }
    assert!(FileSkills::folders(fs.clone(), vec!["../voice".into()]).is_err());
    fs.seed(
        "voice/SKILL.md",
        b"---\nname: voice\n---\nNo description.\n",
    );
    assert!(source.list().is_err());
    fs.seed("voice/SKILL.md", &[b'x'; 131_073]);
    assert!(source.read("voice/SKILL.md").is_err());
    fs.seed("voice/SKILL.md", &[0xff]);
    assert!(source.read("voice/SKILL.md").is_err());
}

#[test]
fn styles_are_six_complete_distinct_recipes_and_a_folder_can_supply_another() {
    let styles = BuiltinStyles;
    let list = styles.list().unwrap();
    assert_eq!(list.len(), 6);
    for meta in list {
        let style = styles.read(&meta.name).unwrap();
        assert!(!style.description.is_empty());
        for role in ["background", "surface", "text", "muted", "accent", "border"] {
            assert!(style.palette.contains_key(role), "{}: {role}", style.name);
        }
        assert!(!style.typography.body.is_empty());
        assert!(!style.typography.heading.is_empty());
        assert!(style.radius.is_finite() && style.radius >= 0.0);
        assert!(style.density.gap > 0.0 && style.density.control_height >= 32.0);
    }
    assert!(styles.read("missing").is_err());
    let fs = Arc::new(MemFs::new());
    let recipe = include_str!("../../../packages/styles/builtin/quiet.json");
    fs.seed("custom.json", recipe.as_bytes());
    let folders = FolderStyles::new(fs.clone(), vec!["custom.json".into()]).unwrap();
    assert_eq!(folders.list().unwrap().len(), 1);
    assert_eq!(
        folders.read("quiet").unwrap(),
        styles.read("quiet").unwrap()
    );
    let bad = recipe.replace("#F8FAFC", "invalid");
    fs.seed("custom.json", bad.as_bytes());
    assert!(folders.list().is_err());
}

#[test]
fn skills_and_styles_tools_work_without_an_open_document_and_validate_inputs() {
    let registry = Registry::standard();
    for name in ["list_skills", "read_skill", "get_style"] {
        let tool = registry.get(name).expect(name);
        assert!(tool.read_only() && !tool.destructive());
        assert_eq!(
            call(&registry, name, json!({"unknown": true}))
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput
        );
    }
    let list = call(&registry, "list_skills", json!({})).unwrap();
    assert_eq!(list["skills"].as_array().unwrap().len(), 7);
    assert_eq!(call(&registry, "read_skill", json!({})).unwrap(), list);
    let read = call(&registry, "read_skill", json!({"path": "execute.md"})).unwrap();
    assert!(read["content"].as_str().unwrap().contains("doc.apiVersion"));
    assert_eq!(
        call(&registry, "read_skill", json!({"path": "execute"})).unwrap(),
        read
    );
    assert_eq!(
        call(&registry, "get_style", json!({})).unwrap()["styles"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let style = call(&registry, "get_style", json!({"name": "quiet"})).unwrap();
    assert!(style["style"]["palette"].is_object());
    for (name, input, field) in [
        ("read_skill", json!({"path": "missing"}), "path"),
        ("get_style", json!({"name": "missing"}), "name"),
    ] {
        let error = call(&registry, name, input).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.path(), Some(field));
    }
}

#[test]
fn configured_sources_are_visible_through_the_same_registry() {
    let fs = Arc::new(MemFs::new());
    fs.seed(
        "voice/SKILL.md",
        b"---\nname: voice\ndescription: TEST DATA\n---\nInstructions.\n",
    );
    let skills = StackedSkills::new(vec![
        Rc::new(BuiltinSkills),
        Rc::new(FileSkills::folders(fs, vec!["voice".into()]).unwrap()),
    ]);
    let registry = Registry::with_sources(Rc::new(skills), Rc::new(BuiltinStyles));
    assert_eq!(
        call(&registry, "list_skills", json!({})).unwrap()["skills"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
    assert!(
        call(&registry, "read_skill", json!({"path":"voice/SKILL.md"})).unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("Instructions.")
    );
}

#[test]
fn content_sources_pass_the_shared_port_contracts() {
    vellum_contracts::suites::content::skills(&BuiltinSkills);
    vellum_contracts::suites::content::styles(&BuiltinStyles);
}

#[test]
fn later_sources_own_duplicate_paths_and_names_without_stale_fallback() {
    use vellum_contracts::fakes::{MemorySkills, MemoryStyles};
    use vellum_core::ports::{ContentError, SkillGroup};
    let meta = BuiltinSkills.list().unwrap().remove(0);
    let user_meta = vellum_core::ports::SkillMeta {
        group: SkillGroup::User,
        ..meta.clone()
    };
    let source = StackedSkills::new(vec![
        Rc::new(BuiltinSkills),
        Rc::new(MemorySkills(vec![(
            user_meta.clone(),
            "TEST DATA override".into(),
        )])),
    ]);
    assert_eq!(source.list().unwrap().len(), 7);
    assert_eq!(source.list().unwrap().last().unwrap(), &user_meta);
    assert_eq!(source.read(&meta.path).unwrap(), "TEST DATA override");
    struct Unreadable(vellum_core::ports::SkillMeta);
    impl SkillSource for Unreadable {
        fn list(&self) -> Result<Vec<vellum_core::ports::SkillMeta>, ContentError> {
            Ok(vec![self.0.clone()])
        }
        fn read(&self, _: &str) -> Result<String, ContentError> {
            Err(ContentError::Unavailable("TEST DATA removed".into()))
        }
    }
    let source = StackedSkills::new(vec![Rc::new(BuiltinSkills), Rc::new(Unreadable(user_meta))]);
    assert!(matches!(
        source.read(&meta.path),
        Err(ContentError::Unavailable(_))
    ));
    let mut custom = BuiltinStyles.read("quiet").unwrap();
    custom.description = "TEST DATA override".into();
    let styles = vellum::content::StackedStyles::new(vec![
        Rc::new(BuiltinStyles),
        Rc::new(MemoryStyles(vec![custom.clone()])),
    ]);
    assert_eq!(styles.list().unwrap().len(), 6);
    assert_eq!(styles.read("quiet").unwrap(), custom);
}

#[test]
fn skill_metadata_supports_quoted_and_block_scalars_without_interpreting_permissions() {
    let fs = Arc::new(MemFs::new());
    fs.seed(
        "voice/SKILL.md",
        b"---\n# TEST DATA comment\n\nname: voice\ndescription: TEST DATA\n---\nInstructions.\n",
    );
    let with_comments = FileSkills::folders(fs.clone(), vec!["voice".into()]).unwrap();
    assert_eq!(with_comments.list().unwrap()[0].name, "voice");
    fs.seed("voice/SKILL.md", b"---\r\nname: 'voice'\r\ndescription: >-\r\n  TEST DATA first\r\n  second\r\nallowed-tools: [anything]\r\n---\r\nInstructions.\r\n");
    let source = FileSkills::folders(fs.clone(), vec!["voice".into()]).unwrap();
    assert_eq!(
        source.list().unwrap()[0].description,
        "TEST DATA first second"
    );
    fs.seed(
        "voice/SKILL.md",
        b"---\nname: \"voice\"\ndescription: \"TEST DATA: a \\\"quote\\\"\"\n---\nInstructions.\n",
    );
    assert_eq!(
        source.list().unwrap()[0].description,
        "TEST DATA: a \"quote\""
    );
    for metadata in [
        "name: voice\nname: other\ndescription: TEST DATA",
        "name: voice\ndescription: []",
        "name: voice\ndescription: ''",
    ] {
        fs.seed(
            "voice/SKILL.md",
            format!("---\n{metadata}\n---\nInstructions.\n").as_bytes(),
        );
        assert!(source.list().is_err(), "{metadata}");
    }
}

#[test]
fn documented_execute_examples_run_on_the_real_api() {
    use std::cell::RefCell;
    use vellum_contracts::{
        fakes::{InMemoryDocStore, SequentialIds},
        samples,
    };
    use vellum_core::{
        doc_api::{self, ExecuteContext, ExecuteRequest},
        guard::GuardState,
        ports::Timestamp,
    };
    let doc = serde_json::from_value(
        json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum 0.0.0"},
        "pages":[{"id":"pageScreen1", "name":"Screens", "children":[]}]}),
    )
    .unwrap();
    let skills = BuiltinSkills;
    let mut examples = 0;
    for meta in skills.list().unwrap() {
        let body = skills.read(&meta.path).unwrap();
        for part in body.split("```javascript\n").skip(1) {
            let code = part.split("```").next().unwrap();
            let mut store = InMemoryDocStore::new(&doc, 1);
            let output = doc_api::execute(
                &vellum_script_quickjs::QuickJsRuntime::new(|| 0, 1),
                &mut store,
                ExecuteRequest {
                    source: code,
                    origin: vellum_types::Origin::new(vellum_types::Actor::Agent {
                        agent: samples::teal(),
                    }),
                    validate_only: false,
                    limits: doc_api::EXECUTE_LIMITS,
                },
                ExecuteContext {
                    ids: Rc::new(RefCell::new(SequentialIds::new())),
                    services: common::scene(),
                    guard: &mut GuardState::default(),
                    now: Timestamp(0),
                },
            )
            .unwrap_or_else(|e| panic!("{}: {e:?}", meta.path));
            assert!(output.committed, "{} must produce a change", meta.path);
            examples += 1;
        }
    }
    assert_eq!(examples, 5, "execute plus four workflow examples");
}

#[test]
fn chart_script_node_example_evaluates_deterministically_with_declared_inputs() {
    use vellum_contracts::fakes::SequentialIds;
    use vellum_core::{resolver::variables::Theme, script_nodes};
    let body = BuiltinSkills
        .read("workflows/make-a-chart-with-a-script.md")
        .unwrap();
    let code = body
        .split("```vellum-script\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let node = serde_json::from_value(
        json!({"type":"script", "id":"testScript1", "name":"TEST DATA chart",
        "src":"charts/bars.js", "width":320, "height":180}),
    )
    .unwrap();
    let doc = design();
    let runtime = vellum_script_quickjs::QuickJsRuntime::new(|| 0, 1);
    let run = || {
        script_nodes::evaluate(
            &runtime,
            &doc,
            &node,
            code,
            &Theme::defaults(&doc),
            &mut SequentialIds::new(),
        )
        .unwrap()
    };
    let first = run();
    assert_eq!(first.nodes.len(), 3);
    assert_eq!(first.nodes, run().nodes);
    assert_eq!(first.header.inputs[0].name, "count");
}
