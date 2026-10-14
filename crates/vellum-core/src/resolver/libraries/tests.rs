//! TEST DATA: library parity, relative paths, stale cache and document paste.
use super::*;
use crate::resolver::components::{
    Resolver,
    tests::{doc, instance},
};
use serde_json::json;
use std::cell::RefCell;
use vellum_types::{ComponentId, Node, NodeId, VariableId};
struct Source(RefCell<BTreeMap<String, Document>>);
impl LibrarySource for Source {
    fn load(&self, path: &str) -> Result<Option<Document>, String> {
        Ok(self.0.borrow().get(path).cloned())
    }
}
#[test]
fn relative_paths_and_missing_cache_recovery() {
    let mut lib = doc();
    lib.meta.kind = DocumentKind::Library;
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/ui.lib.vellum".into(),
        lib.clone(),
    )])));
    let mut libs = Libraries::new(&source);
    let mut d = Document::new("TEST DATA target");
    d.imports.insert("ui".into(), "../ui.lib.vellum".into());
    let n = instance(
        "a",
        "card",
        json!({"library":"ui","overrides":{"root/label":{"props":{"width":77}}}}),
    );
    let mut r = Resolver::new();
    let x = r.resolve_node(&d, "/repo/views/main.vellum", &n, &mut libs);
    assert!(x.all_problems().is_empty());
    assert!(x.missing_libraries.is_empty());
    source.0.borrow_mut().clear();
    let x = r.resolve_node(&d, "/repo/views/main.vellum", &n, &mut libs);
    assert_eq!(x.missing_libraries, vec!["/repo/ui.lib.vellum"]);
    assert_eq!(x.children.len(), 2);
    lib.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()
        .remove(1);
    source
        .0
        .borrow_mut()
        .insert("/repo/new.lib.vellum".into(), lib);
    d.imports.insert("ui".into(), "../new.lib.vellum".into());
    let x = r.resolve_node(&d, "/repo/views/main.vellum", &n, &mut libs);
    assert!(x.missing_libraries.is_empty());
    assert_eq!(x.children.len(), 1);
    assert_eq!(
        x.children[0].value.node.width(),
        &vellum_types::Size::Fixed(77.0.into())
    );
}
#[test]
fn missing_without_cache_and_invalid_import_are_explicit() {
    let source = Source(RefCell::new(BTreeMap::new()));
    let mut libs = Libraries::new(&source);
    let mut d = Document::new("TEST DATA target");
    for path in [
        "ui.lib.vellum",
        "/absolute.lib.vellum",
        "https://host/file.lib.vellum",
    ] {
        d.imports.insert("ui".into(), path.into());
        let x = Resolver::new().resolve_node(
            &d,
            "/repo/main.vellum",
            &instance("a", "card", json!({"library":"ui"})),
            &mut libs,
        );
        assert!(!x.all_problems().is_empty());
    }
}
#[test]
fn library_variables_are_scoped_and_slot_payload_uses_owner() {
    let mut lib = doc();
    lib.meta.kind = DocumentKind::Library;
    lib.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA size","type":"number","values":[{"value":11}]}}),
    )
    .unwrap();
    lib.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()[0] = crate::resolver::components::tests::node(
        crate::resolver::components::tests::rect("label", json!({"$var":"size"})),
    );
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/ui.lib.vellum".into(),
        lib,
    )])));
    let mut libs = Libraries::new(&source);
    let mut d = Document::new("TEST DATA target");
    d.imports.insert("ui".into(), "ui.lib.vellum".into());
    d.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA size","type":"number","values":[{"value":91}]}}),
    )
    .unwrap();
    let payload = crate::resolver::components::tests::rect("owned", json!({"$var":"size"}));
    let n = instance(
        "a",
        "card",
        json!({"library":"ui","slotContent":{"slot":[payload]}}),
    );
    let x = Resolver::new().resolve_node(&d, "/repo/main.vellum", &n, &mut libs);
    assert_eq!(
        x.children[0].value.node.width(),
        &vellum_types::Size::Fixed(11.0.into())
    );
    assert_eq!(
        x.children[1].children[0].value.node.width(),
        &vellum_types::Size::Fixed(91.0.into())
    );
    assert!(x.all_problems().is_empty());
}
#[test]
fn paste_link_copies_and_variable_decisions_are_ops() {
    let mut source = doc();
    source.meta.kind = DocumentKind::Library;
    source.variables = serde_json::from_value(
        json!({"size":{"name":"Space/base","type":"number","values":[{"value":12}]}}),
    )
    .unwrap();
    source
        .components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()[0] = crate::resolver::components::tests::node(
        crate::resolver::components::tests::rect("label", json!({"$var":"size"})),
    );
    let mut target = doc();
    target.variables = serde_json::from_value(
        json!({"existing":{"name":"Space/base","type":"number","values":[{"value":99}]}}),
    )
    .unwrap();
    let nodes = vec![instance("copy", "card", json!({}))];
    let request = PasteRequest {
        source: &source,
        source_path: "/repo/ui.lib.vellum",
        target_path: "/repo/views/main.vellum",
        nodes: &nodes,
        to: crate::ops::Parent::Page(target.pages[0].id.clone()),
        index: 0,
        mode: PasteMode::Copies,
        variables: BTreeMap::new(),
    };
    let mut ids = TestIds(0);
    assert!(matches!(
        plan_paste(&target, &request, &mut ids),
        Err(PasteError::VariableClash { .. })
    ));
    let mut request = request;
    request.variables.insert(
        VariableId::new("size").unwrap(),
        VariableChoice::UseExisting,
    );
    let plan = plan_paste(&target, &request, &mut ids).unwrap();
    assert!(!plan.ops.is_empty());
    let next = apply_plan(&target, plan.ops);
    let pasted = &next.pages[0].children[0];
    let x = Resolver::new().resolve_node(
        &next,
        "/repo/main.vellum",
        pasted,
        &mut Libraries::new(&crate::resolver::components::tests::Empty),
    );
    assert_eq!(
        x.children[0].value.node.width(),
        &vellum_types::Size::Fixed(99.0.into())
    );
    request.variables.insert(
        VariableId::new("size").unwrap(),
        VariableChoice::AddRenamed("Space/imported".into()),
    );
    let next = apply_plan(
        &target,
        plan_paste(&target, &request, &mut ids).unwrap().ops,
    );
    assert!(
        next.variables
            .values()
            .any(|v| v.name() == "Space/imported")
    );
    request.mode = PasteMode::Link { alias: "ui".into() };
    let next = apply_plan(
        &target,
        plan_paste(&target, &request, &mut ids).unwrap().ops,
    );
    assert_eq!(next.imports["ui"], "../ui.lib.vellum");
    let Node::Instance(n) = &next.pages[0].children[0] else {
        panic!()
    };
    assert_eq!(n.library.as_deref(), Some("ui"));
    assert_eq!(n.component_id.as_str(), "card");
}
struct TestIds(u64);
impl crate::ports::IdGen for TestIds {
    fn node_id(&mut self) -> NodeId {
        self.0 += 1;
        NodeId::new(format!("paste{}", self.0)).unwrap()
    }
}
fn apply_plan(doc: &Document, ops: Vec<crate::ops::Op>) -> Document {
    let origin = crate::ops::tests::common::alice();
    let plan = crate::ops::plan(
        doc,
        &crate::ops::Transaction::new(origin, ops),
        &mut TestIds(1000),
    )
    .unwrap();
    let mut next = doc.clone();
    crate::ops::apply_edits(&mut next, &plan.edits, crate::ops::tests::common::alice()).unwrap();
    next
}

#[test]
fn linked_paste_is_valid_without_a_local_definition_even_when_library_is_missing() {
    let mut source = doc();
    source.meta.kind = DocumentKind::Library;
    let mut target = Document::new("TEST DATA destination");
    target.pages = doc().pages;
    let nodes = vec![instance("owned", "card", json!({}))];
    let request = PasteRequest {
        source: &source,
        source_path: "/repo/ui.lib.vellum",
        target_path: "/repo/main.vellum",
        nodes: &nodes,
        to: crate::ops::Parent::Page(target.pages[0].id.clone()),
        index: 0,
        mode: PasteMode::Link { alias: "ui".into() },
        variables: BTreeMap::new(),
    };
    let plan = plan_paste(&target, &request, &mut TestIds(0)).unwrap();
    let pasted = apply_plan(&target, plan.ops);
    assert!(pasted.components.is_empty());
    assert!(
        crate::ops::check_document(&pasted).is_empty(),
        "{:?}",
        crate::ops::check_document(&pasted)
    );
}

#[test]
fn copies_preserve_slot_suggestions_theme_aliases_props_and_metadata() {
    let mut source = doc();
    source.themes = BTreeMap::from([("Mode".into(), vec!["Light".into(), "Dark".into()])]);
    source.variables=serde_json::from_value(json!({"base":{"name":"Space/base","type":"number","values":[{"value":12},{"value":24,"when":{"Mode":"Dark"}}]},"alias":{"name":"Space/alias","type":"number","values":[{"$alias":"base"}]}})).unwrap();
    let c = source
        .components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap();
    c.root.children_mut().unwrap()[0] = crate::resolver::components::tests::node(
        crate::resolver::components::tests::rect("label", json!({"$var":"alias"})),
    );
    c.props=serde_json::from_value(json!({"size":{"type":"number","default":39,"bindings":[{"node":"root/label","property":"height"}]}})).unwrap();
    let mut target = Document::new("TEST DATA destination");
    target.pages = doc().pages;
    let nodes = vec![instance(
        "owned",
        "card",
        json!({"metadata":{"componentId":"card","id":"label","$var":"alias"}}),
    )];
    let req = PasteRequest {
        source: &source,
        source_path: "/repo/source.vellum",
        target_path: "/repo/dest.vellum",
        nodes: &nodes,
        to: crate::ops::Parent::Page(target.pages[0].id.clone()),
        index: 0,
        mode: PasteMode::Link {
            alias: "ignored".into(),
        },
        variables: BTreeMap::new(),
    };
    let next = apply_plan(
        &target,
        plan_paste(&target, &req, &mut TestIds(0)).unwrap().ops,
    );
    let n = &next.pages[0].children[0];
    let json = serde_json::to_value(n).unwrap();
    assert_eq!(
        json["metadata"],
        json!({"componentId":"card","id":"label","$var":"alias"})
    );
    let x = Resolver::new().resolve_node(
        &next,
        "/repo/dest.vellum",
        n,
        &mut Libraries::new(&crate::resolver::components::tests::Empty),
    );
    assert!(x.all_problems().is_empty(), "{:?}", x.all_problems());
    assert_eq!(
        x.children[0].value.node.width(),
        &vellum_types::Size::Fixed(12.0.into())
    );
    assert_eq!(
        x.children[0].value.node.height(),
        &vellum_types::Size::Fixed(39.0.into())
    );
    assert_eq!(
        next.components.len(),
        2,
        "slot suggestions need their definitions"
    );
    assert!(
        next.components
            .contains_key(&x.children[1].slot.as_ref().unwrap().suggested[0])
    );
    assert_eq!(next.themes["Mode"], vec!["Light", "Dark"]);
}
#[test]
fn imported_overrides_use_destination_variables_even_with_colliding_library_ids() {
    let mut lib = doc();
    lib.meta.kind = DocumentKind::Library;
    lib.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA library size","type":"number","values":[{"value":11}]}}),
    )
    .unwrap();
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/ui.lib.vellum".into(),
        lib,
    )])));
    let mut libraries = Libraries::new(&source);
    let mut d = Document::new("TEST DATA destination");
    d.imports.insert("ui".into(), "ui.lib.vellum".into());
    d.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA local size","type":"number","values":[{"value":91}]}}),
    )
    .unwrap();
    let n = instance(
        "a",
        "card",
        json!({"library":"ui","overrides":{"root/label":{"props":{"width":{"$var":"size"}}}}}),
    );
    let tree = Resolver::new().resolve_node(&d, "/repo/main.vellum", &n, &mut libraries);
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    assert_eq!(
        tree.children[0].value.node.width(),
        &vellum_types::Size::Fixed(91.0.into())
    );
}
#[test]
fn unreadable_and_nonlibrary_replacements_keep_a_flagged_last_known_copy() {
    struct Bad;
    impl LibrarySource for Bad {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            Err("TEST DATA unreadable".into())
        }
    }
    let mut lib = doc();
    lib.meta.kind = DocumentKind::Library;
    let mut libraries = Libraries::new(&Bad);
    libraries.seed("/repo/ui.lib.vellum", lib).unwrap();
    let got = libraries.get("/repo/main.vellum", "ui.lib.vellum").unwrap();
    assert!(got.missing);
    assert!(got.document.is_some());
    assert_eq!(got.problem.as_deref(), Some("TEST DATA unreadable"));
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/ui.lib.vellum".into(),
        doc(),
    )])));
    let mut libraries = Libraries::new(&source);
    let got = libraries.get("/repo/main.vellum", "ui.lib.vellum").unwrap();
    assert!(got.missing);
    assert!(got.document.is_none());
}

#[test]
fn transitive_relative_libraries_keep_source_scope_and_missing_flags() {
    let mut a = doc();
    a.meta.kind = DocumentKind::Library;
    a.imports
        .insert("shared".into(), "../shared/b.lib.vellum".into());
    a.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()
        .push(instance("nested", "badge", json!({"library":"shared"})));
    let mut b = doc();
    b.meta.kind = DocumentKind::Library;
    let source = Source(RefCell::new(BTreeMap::from([
        ("/repo/lib/a.lib.vellum".into(), a),
        ("/repo/shared/b.lib.vellum".into(), b),
    ])));
    let mut libraries = Libraries::new(&source);
    let mut d = Document::new("TEST DATA destination");
    d.imports.insert("ui".into(), "lib/a.lib.vellum".into());
    let n = instance("outer", "card", json!({"library":"ui"}));
    let mut resolver = Resolver::new();
    let tree = resolver.resolve_node(&d, "/repo/main.vellum", &n, &mut libraries);
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    assert_eq!(tree.children[2].source, "/repo/shared/b.lib.vellum");
    source.0.borrow_mut().remove("/repo/shared/b.lib.vellum");
    let tree = resolver.resolve_node(&d, "/repo/main.vellum", &n, &mut libraries);
    assert_eq!(tree.missing_libraries, vec!["/repo/shared/b.lib.vellum"]);
    assert_eq!(tree.children[2].children.len(), 1);
}
#[test]
fn copied_icons_and_user_input_keys_are_not_library_aliases() {
    let mut source = doc();
    source
        .imports
        .insert("lucide".into(), "lib/icons.lib.vellum".into());
    let mut target = Document::new("TEST DATA destination");
    target.pages = doc().pages;
    target
        .imports
        .insert("lucide".into(), "other.lib.vellum".into());
    let icon = crate::resolver::components::tests::node(
        json!({"type":"icon","id":"icon","name":"TEST DATA icon","width":16,"height":16,"library":"lucide","icon":"star","weight":400}),
    );
    let nodes = vec![icon];
    let req = PasteRequest {
        source: &source,
        source_path: "/repo/source.vellum",
        target_path: "/repo/dest.vellum",
        nodes: &nodes,
        to: crate::ops::Parent::Page(target.pages[0].id.clone()),
        index: 0,
        mode: PasteMode::Copies,
        variables: BTreeMap::new(),
    };
    let next = apply_plan(
        &target,
        plan_paste(&target, &req, &mut TestIds(0)).unwrap().ops,
    );
    let Node::Icon(icon) = &next.pages[0].children[0] else {
        panic!()
    };
    assert_eq!(icon.library, "lucide");
}

#[test]
fn replacing_an_imported_root_uses_the_importers_variable_scope() {
    let mut lib = doc();
    lib.meta.kind = DocumentKind::Library;
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/ui.lib.vellum".into(),
        lib,
    )])));
    let mut d = Document::new("TEST DATA destination");
    d.imports.insert("ui".into(), "ui.lib.vellum".into());
    d.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA size","type":"number","values":[{"value":48}]}}),
    )
    .unwrap();
    let mut replacement = crate::resolver::components::tests::rect("replacement", json!(10));
    replacement["cornerRadius"] = json!({"$var":"size"});
    let n = instance(
        "a",
        "card",
        json!({"library":"ui","overrides":{"root":{"replace":replacement}}}),
    );
    let tree =
        Resolver::new().resolve_node(&d, "/repo/main.vellum", &n, &mut Libraries::new(&source));
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    // Instance hug sizing remains its explicit choice; the replacement's other bound properties use the importer.
    assert_eq!(tree.source, "/repo/main.vellum");
    assert_eq!(
        serde_json::to_value(&tree.value.node).unwrap()["cornerRadius"],
        json!(48.0)
    );
}

#[test]
fn deep_library_overrides_keep_importer_variables_and_local_component_links() {
    let mut lib = doc();
    lib.meta.kind = DocumentKind::Library;
    lib.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()
        .push(instance("inner", "badge", json!({})));
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/ui.lib.vellum".into(),
        lib,
    )])));
    let mut d = doc();
    d.imports.insert("ui".into(), "ui.lib.vellum".into());
    d.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA local size","type":"number","values":[{"value":91}]}}),
    )
    .unwrap();
    let n = instance(
        "outer",
        "card",
        json!({"library":"ui","overrides":{"root/inner/badgeRoot/dot":{"props":{"width":{"$var":"size"}}}}}),
    );
    let mut resolver = Resolver::new();
    let mut libraries = Libraries::new(&source);
    let tree = resolver.resolve_node(&d, "/repo/main.vellum", &n, &mut libraries);
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    assert_eq!(
        tree.children[2].children[0].value.node.width(),
        &vellum_types::Size::Fixed(91.0.into())
    );
    let replacement = instance("owned", "card", json!({}));
    let n = instance(
        "outer",
        "card",
        json!({"library":"ui","overrides":{"root/inner/badgeRoot/dot":{"replace":replacement}}}),
    );
    let tree = resolver.resolve_node(&d, "/repo/main.vellum", &n, &mut libraries);
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    assert_eq!(tree.children[2].children[0].source, "/repo/main.vellum");
}

#[test]
fn parity_documents_resolve_without_mutating_their_source() {
    let components: Document = serde_json::from_str(include_str!(
        "../../../../../fixtures/resolver/components.vellum"
    ))
    .unwrap();
    let imported: Document = serde_json::from_str(include_str!(
        "../../../../../fixtures/resolver/imports.vellum"
    ))
    .unwrap();
    let library: Document = serde_json::from_str(include_str!(
        "../../../../../fixtures/resolver/library.lib.vellum"
    ))
    .unwrap();
    let source = Source(RefCell::new(BTreeMap::from([(
        "/repo/library.lib.vellum".into(),
        library,
    )])));
    let mut resolver = Resolver::new();
    let mut libraries = Libraries::new(&source);
    for doc in [components, imported] {
        let before = doc.clone();
        assert!(
            crate::ops::check_document(&doc).is_empty(),
            "{:?}",
            crate::ops::check_document(&doc)
        );
        let roots = resolver.resolve_page(&doc, "/repo/main.vellum", &doc.pages[0], &mut libraries);
        assert!(!roots.is_empty());
        for root in roots {
            assert!(root.all_problems().is_empty(), "{:?}", root.all_problems());
        }
        assert_eq!(doc, before);
    }
}

#[test]
fn imported_component_ops_do_not_silently_use_a_same_id_local_definition() {
    let mut d = doc();
    d.imports.insert("ui".into(), "ui.lib.vellum".into());
    d.pages[0]
        .children
        .push(instance("linked", "card", json!({"library":"ui"})));
    let result = crate::ops::tests::common::planned(
        &d,
        vec![crate::ops::Op::Detach {
            id: NodeId::new("linked").unwrap(),
        }],
    );
    assert!(
        matches!(
            result,
            Err(crate::ops::OpError {
                kind: crate::ops::OpErrorKind::MissingInput(_),
                ..
            })
        ),
        "an unresolved imported instance must never use a local same-id component"
    );
}
