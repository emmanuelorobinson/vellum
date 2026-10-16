//! TEST DATA. The real sandbox and the real op layer; only storage/clock/ids are fakes.
use std::{cell::RefCell, rc::Rc};
use vellum_contracts::fakes::{InMemoryDocStore, SequentialIds};
use vellum_core::{
    doc_api::{self, ExecuteContext, ExecuteRequest, NoServices},
    guard::GuardState,
    ports::{DocStore, ScriptLimits, ScriptValue, Timestamp},
    script_nodes::{self, Header},
};
use vellum_script_quickjs::QuickJsRuntime;
use vellum_types::{Actor, DeviceId, Document, Origin};

fn origin() -> Origin {
    Origin {
        actor: Actor::Person {
            device: DeviceId::new("00000000000000000000000001").unwrap(),
        },
        session: None,
        turn: None,
    }
}
fn doc() -> Document {
    serde_json::from_value(serde_json::json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"TEST DATA"},"pages":[{"id":"page","name":"TEST DATA"}]})).unwrap()
}
fn run(
    code: &str,
    dry: bool,
    store: &mut InMemoryDocStore,
) -> Result<doc_api::ExecuteOutput, vellum_core::ports::ScriptDiagnostic> {
    doc_api::execute(
        &QuickJsRuntime::new(|| 0, 1),
        store,
        ExecuteRequest {
            source: code,
            origin: origin(),
            validate_only: dry,
            limits: doc_api::EXECUTE_LIMITS,
        },
        ExecuteContext {
            ids: Rc::new(RefCell::new(SequentialIds::new())),
            services: Rc::new(NoServices),
            guard: &mut GuardState::default(),
            now: Timestamp(0),
        },
    )
}

#[test]
fn doc_api_atomic_read_your_writes_and_one_undo() {
    let before = doc();
    let mut store = InMemoryDocStore::new(&before, 1);
    let out = run("const p=doc.page('TEST DATA'); const n=doc.insert(p,{type:'frame',width:100,height:50}); doc.insert(n,{type:'rect',width:5,height:5}); doc.update(n,{name:'TEST DATA changed'}); return [doc.apiVersion,doc.get(n).name,doc.query({type:'rect'}).length];", false, &mut store).unwrap();
    assert_eq!(
        out.value,
        ScriptValue::List(vec![
            ScriptValue::Text("1.0".into()),
            ScriptValue::Text("TEST DATA changed".into()),
            ScriptValue::Number(1.)
        ])
    );
    assert!(out.committed);
    store.undo(&origin().actor).unwrap();
    assert_eq!(store.snapshot(), before);
    assert!(store.undo(&origin().actor).is_none());
}
#[test]
fn doc_api_failure_and_dry_run_leave_store_unchanged() {
    let before = doc();
    let mut store = InMemoryDocStore::new(&before, 1);
    let out = run(
        "doc.insert(doc.page('TEST DATA'),{type:'rect',width:5,height:5});",
        true,
        &mut store,
    )
    .unwrap();
    assert!(!out.committed);
    assert_eq!(out.document.pages[0].children.len(), 1);
    assert_eq!(store.snapshot(), before);
    let error=run("doc.insert(doc.page('TEST DATA'),{type:'rect',width:5,height:5});\n  doc.update('missing',{width:8});",false,&mut store).unwrap_err();
    assert_eq!(error.line, Some(2));
    assert!(error.column.is_some());
    assert!(error.excerpt.contains("doc.update"));
    assert_eq!(store.snapshot(), before);
}
#[test]
fn doc_api_copy_cost_counts_descendants_before_call() {
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let request = ExecuteRequest {
        source: "const p=doc.page('TEST DATA'); const n=doc.insert(p,{type:'frame',width:10,height:10,children:[{type:'rect',width:1,height:1}]}); doc.copy(n,p);",
        origin: origin(),
        validate_only: false,
        limits: ScriptLimits {
            max_nodes: 3,
            ..doc_api::EXECUTE_LIMITS
        },
    };
    let err = doc_api::execute(
        &QuickJsRuntime::new(|| 0, 1),
        &mut store,
        request,
        ExecuteContext {
            ids: Rc::new(RefCell::new(SequentialIds::new())),
            services: Rc::new(NoServices),
            guard: &mut GuardState::default(),
            now: Timestamp(0),
        },
    )
    .unwrap_err();
    assert_eq!(err.error, vellum_core::ports::ScriptError::NodeLimit);
    assert_eq!(store.snapshot(), doc());
}
#[test]
fn script_nodes_headers_and_derived_output() {
    let source = "/**\n * @schema 1\n * @input count: number(min=1,max=10,step=1) = 2\n * @input label: string = \"TEST DATA\"\n * @input enabled: boolean = true\n * @input tint: color = \"#112233\"\n * @input mode: enum(\"a\",\"b\") = \"a\"\n * @input target: ref = \"page\"\n */\nreturn Array.from({length:doc.inputs.count},()=>({type:'rect',width:10,height:10}));";
    let header = Header::parse(source).unwrap();
    assert_eq!(header.inputs.len(), 6);
    let node=serde_json::from_value(serde_json::json!({"type":"script","id":"script","name":"TEST DATA","src":"chart.js","width":100,"height":100,"clip":true})).unwrap();
    let before = doc();
    let output = script_nodes::evaluate(
        &QuickJsRuntime::new(|| 0, 1),
        &before,
        &node,
        source,
        &vellum_core::resolver::variables::Theme::defaults(&before),
        &mut SequentialIds::new(),
    )
    .unwrap();
    assert_eq!(output.nodes.len(), 2);
    assert!(output.clip);
    assert_eq!(before, doc());
    assert!(Header::parse("// @schema 2\nreturn [];").is_err());
    assert!(Header::parse("// @input x: number(min=5) = 2").is_err());
}

#[test]
fn script_nodes_convert_is_one_transaction_and_preserves_appearance() {
    use vellum_core::{
        ops::{self, Inputs, Op, Transaction},
        ports::ScriptError,
    };
    use vellum_types::NodeId;
    let mut before = doc();
    let node=serde_json::from_value(serde_json::json!({"type":"script","id":"script","name":"TEST DATA","src":"chart.js","width":100,"height":100,"opacity":0.5,"clip":true})).unwrap();
    before.pages[0].children.push(node);
    let out = script_nodes::evaluate(
        &QuickJsRuntime::new(|| 0, 1),
        &before,
        &before.pages[0].children[0],
        "return [{type:'rect',width:2,height:3}];",
        &vellum_core::resolver::variables::Theme::defaults(&before),
        &mut SequentialIds::new(),
    )
    .unwrap();
    let mut store = InMemoryDocStore::new(&before, 1);
    let id = NodeId::new("script").unwrap();
    let mut inputs = Inputs::default();
    inputs.script_outputs.insert(id.clone(), out.nodes);
    ops::apply_with_inputs(
        &mut store,
        &mut SequentialIds::new(),
        Transaction::new(origin(), vec![Op::ConvertScript { id }]),
        &inputs,
    )
    .unwrap();
    let after = store.snapshot();
    let vellum_types::Node::Frame(frame) = &after.pages[0].children[0] else {
        panic!("frame")
    };
    assert!(frame.clip);
    assert_eq!(frame.opacity, vellum_types::Value::Literal(0.5));
    assert_eq!(frame.children.len(), 1);
    store.undo(&origin().actor).unwrap();
    assert_eq!(store.snapshot(), before);
    assert!(store.undo(&origin().actor).is_none());
    let result = script_nodes::evaluate(
        &QuickJsRuntime::new(|| 0, 1),
        &doc(),
        &before.pages[0].children[0],
        "return Array.from({length:1001},()=>({type:'rect',width:1,height:1}));",
        &vellum_core::resolver::variables::Theme::defaults(&before),
        &mut SequentialIds::new(),
    );
    assert_eq!(result.unwrap_err().error, ScriptError::NodeLimit);
}

#[test]
fn doc_api_all_structural_bindings_and_notes_are_visible() {
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let out=run("const p=doc.page('page'); const f=doc.insert(p,{type:'frame',width:100,height:100,agentNotes:'TEST DATA instruction'}); const a=doc.insert(f,{type:'rect',width:1,height:1}); const b=doc.copy(a,p); doc.move(b,f,0); const c=doc.replace(a,{type:'ellipse',width:2,height:2}); doc.delete(c); const component=doc.createComponent(f); const i=doc.instance(p,component,{width:100,height:100}); doc.fillImage(i,'asset'); const icon=doc.icon(p,'test:check',{width:10,height:10}); doc.setVariables({n:{name:'TEST DATA',type:'number',values:[{value:3}]}}); return [doc.get(f).agentNotes,doc.query({scope:p,type:'icon'}).length,doc.get(icon).icon,component];",false,&mut store).unwrap();
    assert!(
        matches!(out.value,ScriptValue::List(ref values) if values[0]==ScriptValue::Text("TEST DATA instruction".into()) && values[1]==ScriptValue::Number(1.))
    );
    store.undo(&origin().actor).unwrap();
    assert_eq!(store.snapshot(), doc());
}

#[test]
fn doc_api_reads_use_current_staged_document_and_services_fail_atomically() {
    struct Read;
    impl doc_api::Services for Read {
        fn read(
            &self,
            method: doc_api::Method,
            document: &Document,
            _: &[ScriptValue],
        ) -> Result<ScriptValue, vellum_core::ports::ScriptError> {
            assert!(matches!(
                method,
                doc_api::Method::FindSpace
                    | doc_api::Method::LayoutProblems
                    | doc_api::Method::Screenshot
                    | doc_api::Method::Export
            ));
            assert_eq!(document.pages[0].children.len(), 1);
            Ok(ScriptValue::Text(format!("{method:?}")))
        }
    }
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let out=doc_api::execute(&QuickJsRuntime::new(||0,1),&mut store,ExecuteRequest {source:"const p=doc.page('page');doc.insert(p,{type:'rect',width:1,height:1});return [doc.findSpace(p,{width:4,height:4}),doc.layoutProblems(p),doc.screenshot(p),doc.export(p,{format:'svg'})];",origin:origin(),validate_only:true,limits:doc_api::EXECUTE_LIMITS},ExecuteContext {ids:Rc::new(RefCell::new(SequentialIds::new())),services:Rc::new(Read),guard:&mut GuardState::default(),now:Timestamp(0)}).unwrap();
    assert!(matches!(out.value,ScriptValue::List(ref v) if v.len()==4));
    assert_eq!(store.snapshot(), doc());
    assert!(
        run(
            "doc.insert('page',{type:'rect',width:1,height:1}); doc.screenshot('page');",
            false,
            &mut store
        )
        .is_err()
    );
    assert!(
        run(
            "doc.insert('page',{type:'rect',width:1,height:1}); throw new Error('TEST DATA');",
            false,
            &mut store
        )
        .is_err()
    );
    assert_eq!(store.snapshot(), doc());
}

#[test]
fn script_nodes_bound_inputs_and_runtime_errors() {
    let mut doc = doc();
    doc.variables = serde_json::from_value(
        serde_json::json!({"count":{"name":"TEST DATA","type":"number","values":[{"value":3}]}}),
    )
    .unwrap();
    let node=serde_json::from_value(serde_json::json!({"type":"script","id":"script","name":"TEST DATA","src":"chart.js","width":100,"height":100,"inputs":{"count":{"$var":"count"}}})).unwrap();
    let source = "// @schema 1\n// @input count: number(min=1,max=3) = 1\nreturn Array.from({length:doc.inputs.count},()=>({type:'rect',width:1,height:1}));";
    let out = script_nodes::evaluate(
        &QuickJsRuntime::new(|| 0, 1),
        &doc,
        &node,
        source,
        &vellum_core::resolver::variables::Theme::defaults(&doc),
        &mut SequentialIds::new(),
    )
    .unwrap();
    assert_eq!(out.nodes.len(), 3);
    let err = script_nodes::evaluate(
        &QuickJsRuntime::new(|| 0, 1),
        &doc,
        &node,
        "// @input count: number = 1\nthrow new Error('TEST DATA failure');",
        &vellum_core::resolver::variables::Theme::defaults(&doc),
        &mut SequentialIds::new(),
    )
    .unwrap_err();
    assert_eq!(err.line, Some(2), "{err:?}");
    assert!(err.excerpt.contains("TEST DATA failure"));
}

#[test]
fn doc_api_scene_reads_and_script_nodes_render_projection() {
    use vellum_contracts::fakes::{FixedMetricText, NaiveSpatialIndex, StackLayout, StaticFonts};
    use vellum_core::doc_api::{RenderRead, Rendered, SceneConfig, SceneServices};
    struct Render;
    impl RenderRead for Render {
        fn render(
            &self,
            output: &vellum_core::display_list::Output,
            _: vellum_core::ports::Rect,
            _: f64,
            format: &str,
        ) -> Result<Rendered, vellum_core::ports::ScriptError> {
            assert!(!output.list.commands.is_empty());
            Ok(Rendered {
                mime: format!("image/{format}"),
                bytes: vec![1, 2, 3],
            })
        }
    }
    struct Libraries;
    impl vellum_core::resolver::libraries::LibrarySource for Libraries {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            Ok(None)
        }
    }
    let config = SceneConfig {
        script_outputs: Default::default(),
        path: "TEST-DATA.vellum".into(),
        libraries: Rc::new(Libraries),
        layout: RefCell::new(Box::new(StackLayout)),
        text: Rc::new(FixedMetricText::default()),
        fonts: Rc::new(StaticFonts::new(&[])),
        renderer: Rc::new(Render),
    };
    let services = Rc::new(SceneServices::new(config, NaiveSpatialIndex::default));
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let out=doc_api::execute(&QuickJsRuntime::new(||0,1),&mut store,ExecuteRequest {source:"const p=doc.page('page'); doc.insert(p,{type:'rect',width:10,height:10,fills:[{type:'solid',color:'#123456'}]}); const space=doc.findSpace(p,{width:5,height:5}); if(space.x<26 && space.x+5>-16 && space.y<26 && space.y+5>-16) throw new Error('occupied'); return [doc.layoutProblems(p),doc.screenshot(p),doc.export(p,{format:'svg'})];",origin:origin(),validate_only:true,limits:doc_api::EXECUTE_LIMITS},ExecuteContext {ids:Rc::new(RefCell::new(SequentialIds::new())),services,guard:&mut GuardState::default(),now:Timestamp(0)}).unwrap();
    assert!(matches!(out.value,ScriptValue::List(ref v) if v.len()==3));
}

#[test]
fn script_nodes_derived_children_reach_resolver_and_layout_and_invalidate() {
    use vellum_core::resolver::{
        Resolver,
        libraries::{Libraries, LibrarySource},
    };
    use vellum_types::NodeId;
    struct Empty;
    impl LibrarySource for Empty {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            Ok(None)
        }
    }
    let mut doc = doc();
    let script=serde_json::from_value(serde_json::json!({"type":"script","id":"script","name":"TEST DATA","src":"chart.js","width":100,"height":100,"clip":true})).unwrap();
    doc.pages[0].children.push(script);
    let before = doc.clone();
    let mut resolver = Resolver::new();
    let mut libraries = Libraries::new(&Empty);
    let output = script_nodes::evaluate(
        &QuickJsRuntime::new(|| 0, 1),
        &doc,
        &doc.pages[0].children[0],
        "return [{type:'rect',width:2,height:3}];",
        &vellum_core::resolver::variables::Theme::defaults(&doc),
        &mut SequentialIds::new(),
    )
    .unwrap();
    resolver.set_script_outputs(
        [(
            ("TEST-DATA.vellum".into(), NodeId::new("script").unwrap()),
            output,
        )]
        .into(),
    );
    let roots = resolver.resolve_page(&doc, "TEST-DATA.vellum", &doc.pages[0], &mut libraries);
    let tree = vellum_core::layout::from_resolved(&roots[0]).unwrap();
    assert_eq!(tree.root.children.len(), 1);
    assert!(tree.root.style.clip);
    resolver.set_script_outputs(Default::default());
    let roots = resolver.resolve_page(&doc, "TEST-DATA.vellum", &doc.pages[0], &mut libraries);
    assert!(roots[0].children.is_empty());
    assert_eq!(doc, before);
}

fn agent_origin() -> Origin {
    Origin {
        actor: Actor::Agent {
            agent: vellum_types::AgentId::new("00000000000000000000000002").unwrap(),
        },
        session: None,
        turn: None,
    }
}
fn as_agent(
    source: &str,
    dry: bool,
    store: &mut InMemoryDocStore,
    guard: &mut GuardState,
) -> Result<doc_api::ExecuteOutput, vellum_core::ports::ScriptDiagnostic> {
    doc_api::execute(
        &QuickJsRuntime::new(|| 0, 1),
        store,
        ExecuteRequest {
            source,
            origin: agent_origin(),
            validate_only: dry,
            limits: doc_api::EXECUTE_LIMITS,
        },
        ExecuteContext {
            ids: Rc::new(RefCell::new(SequentialIds::new())),
            services: Rc::new(NoServices),
            guard,
            now: Timestamp(1000),
        },
    )
}
#[test]
fn doc_api_leases_stage_atomically_and_activity_extends_them() {
    let mut before = doc();
    before.pages[0].children.push(serde_json::from_value(serde_json::json!({"id":"frame","name":"TEST DATA","type":"frame","width":10,"height":10})).unwrap());
    let mut store = InMemoryDocStore::new(&before, 1);
    let mut guard = GuardState::default();
    let id = vellum_types::NodeId::new("frame").unwrap();
    assert!(
        as_agent(
            "doc.claim('frame');throw new Error('TEST DATA');",
            false,
            &mut store,
            &mut guard
        )
        .is_err()
    );
    assert!(guard.leases.holder(&id, Timestamp(1000)).is_none());
    as_agent(
        "doc.claim('frame');doc.update('frame',{name:'TEST DATA changed'});",
        true,
        &mut store,
        &mut guard,
    )
    .unwrap();
    assert!(guard.leases.holder(&id, Timestamp(1000)).is_none());
    assert_eq!(store.snapshot(), before);
    as_agent("doc.claim('frame');", false, &mut store, &mut guard).unwrap();
    let Actor::Agent { agent } = agent_origin().actor else {
        unreachable!()
    };
    // An old lease has 59 seconds left at the call's timestamp; a successful write refreshes it.
    guard
        .leases
        .claim(&id, agent.clone(), &guard.blocked, Timestamp(0))
        .unwrap();
    as_agent(
        "doc.update('frame',{name:'TEST DATA changed'});",
        false,
        &mut store,
        &mut guard,
    )
    .unwrap();
    assert_eq!(guard.leases.holder(&id, Timestamp(60500)), Some(&agent));
    as_agent("doc.release('frame');", false, &mut store, &mut guard).unwrap();
    assert!(guard.leases.holder(&id, Timestamp(1000)).is_none());
    guard.pause_all(Timestamp(1000));
    let snapshot = store.snapshot();
    assert!(
        as_agent(
            "doc.update('frame',{width:3});",
            false,
            &mut store,
            &mut guard
        )
        .is_err()
    );
    assert_eq!(store.snapshot(), snapshot);
}
#[test]
fn doc_api_make_slot_uses_the_component_op_and_registry_names_are_unique() {
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let out=run("const p=doc.page('page');const f=doc.insert(p,{type:'frame',width:10,height:10,children:[{type:'frame',name:'TEST DATA slot',width:3,height:3}]});doc.createComponent(f);const slot=doc.query({type:'frame',name:'TEST DATA slot'})[0];doc.makeSlot(slot.id);return doc.get(slot.id);",true,&mut store).unwrap();
    assert!(matches!(out.value,ScriptValue::Map(ref map) if map.contains_key("slot")));
    let names = doc_api::REGISTRY
        .iter()
        .map(|d| d.name)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), 21);
}

#[test]
fn doc_api_screenshot_of_child_keeps_ancestor_theme_and_world_transform() {
    use vellum_contracts::fakes::{FixedMetricText, NaiveSpatialIndex, StackLayout, StaticFonts};
    use vellum_core::{
        doc_api::{RenderRead, Rendered, SceneConfig, SceneServices, Services},
        ports::Rect,
    };
    struct Render;
    impl RenderRead for Render {
        fn render(
            &self,
            _: &vellum_core::display_list::Output,
            b: Rect,
            _: f64,
            _: &str,
        ) -> Result<Rendered, vellum_core::ports::ScriptError> {
            assert_eq!(b, Rect::new(40., 30., 7., 3.));
            Ok(Rendered {
                mime: "image/png".into(),
                bytes: vec![1],
            })
        }
    }
    struct Libraries;
    impl vellum_core::resolver::libraries::LibrarySource for Libraries {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            Ok(None)
        }
    }
    let mut document = doc();
    document.themes =
        std::collections::BTreeMap::from([("Mode".into(), vec!["Light".into(), "Dark".into()])]);
    document.variables=serde_json::from_value(serde_json::json!({"size":{"type":"number","name":"TEST DATA","values":[{"value":2},{"value":7,"when":{"Mode":"Dark"}}]}})).unwrap();
    document.pages[0].children.push(serde_json::from_value(serde_json::json!({"type":"frame","id":"frame","name":"TEST DATA","width":100,"height":100,"position":{"type":"absolute","x":40,"y":30},"theme":{"Mode":"Dark"},"children":[{"type":"rect","id":"child","name":"TEST DATA","width":{"$var":"size"},"height":3,"fills":[{"type":"solid","color":"#123456"}]}]})).unwrap());
    let services = SceneServices::new(
        SceneConfig {
            script_outputs: Default::default(),
            path: "TEST-DATA.vellum".into(),
            libraries: Rc::new(Libraries),
            layout: RefCell::new(Box::new(StackLayout)),
            text: Rc::new(FixedMetricText::default()),
            fonts: Rc::new(StaticFonts::new(&[])),
            renderer: Rc::new(Render),
        },
        NaiveSpatialIndex::default,
    );
    services
        .read(
            doc_api::Method::Screenshot,
            &document,
            &[ScriptValue::Text("child".into())],
        )
        .unwrap();
}

#[test]
fn script_nodes_changed_inputs_invalidate_cached_output() {
    use vellum_core::resolver::{
        Resolver,
        libraries::{Libraries, LibrarySource},
    };
    struct Empty;
    impl LibrarySource for Empty {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            Ok(None)
        }
    }
    let mut document = doc();
    let mut script:vellum_types::Node=serde_json::from_value(serde_json::json!({"type":"script","id":"script","name":"TEST DATA","src":"chart.js","width":10,"height":10,"inputs":{"count":1}})).unwrap();
    let out=script_nodes::evaluate(&QuickJsRuntime::new(||0,1),&document,&script,"// @input count: number = 1\nreturn Array.from({length:doc.inputs.count},()=>({type:'rect',width:1,height:1}));",&vellum_core::resolver::variables::Theme::defaults(&document),&mut SequentialIds::new()).unwrap();
    if let vellum_types::Node::Script(n) = &mut script {
        n.inputs.insert(
            "count".into(),
            vellum_types::Value::Literal(vellum_types::Scalar::Number(2.)),
        );
    }
    document.pages[0].children.push(script);
    let mut resolver = Resolver::new();
    resolver.set_script_outputs(
        [(
            (
                "TEST-DATA.vellum".into(),
                vellum_types::NodeId::new("script").unwrap(),
            ),
            out,
        )]
        .into(),
    );
    let roots = resolver.resolve_page(
        &document,
        "TEST-DATA.vellum",
        &document.pages[0],
        &mut Libraries::new(&Empty),
    );
    assert!(roots[0].children.is_empty());
    assert!(
        !roots[0].problems.is_empty(),
        "stale output must be explicit"
    );
}

#[test]
fn doc_api_instance_without_options_inherits_definition_dimensions() {
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let out=run("const f=doc.insert('page',{type:'frame',width:12,height:34});const c=doc.createComponent(f);return doc.get(doc.instance('page',c));",true,&mut store).unwrap();
    let ScriptValue::Map(node) = out.value else {
        panic!("node")
    };
    assert_eq!(node.get("width"), Some(&ScriptValue::Number(12.)));
    assert_eq!(node.get("height"), Some(&ScriptValue::Number(34.)));
}

#[test]
fn doc_api_caller_cannot_raise_execute_policy_limits() {
    let mut store = InMemoryDocStore::new(&doc(), 1);
    let error = doc_api::execute(
        &QuickJsRuntime::new(|| 0, 1),
        &mut store,
        ExecuteRequest {
            source: "for(let i=0;i<2001;i++)doc.get('missing');",
            origin: origin(),
            validate_only: false,
            limits: ScriptLimits {
                time_ms: u32::MAX,
                max_ops: u32::MAX,
                max_nodes: u32::MAX,
                memory_mb: u32::MAX,
            },
        },
        ExecuteContext {
            ids: Rc::new(RefCell::new(SequentialIds::new())),
            services: Rc::new(NoServices),
            guard: &mut GuardState::default(),
            now: Timestamp(0),
        },
    )
    .unwrap_err();
    assert_eq!(error.error, vellum_core::ports::ScriptError::OpLimit);
    assert_eq!(store.snapshot(), doc());
}
