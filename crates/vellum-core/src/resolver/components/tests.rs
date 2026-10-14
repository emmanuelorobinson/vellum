//! TEST DATA: component/slot parity fixtures, independent of rendering and disk.
use super::*;
use crate::resolver::libraries::{Libraries, LibrarySource};
use serde_json::{Value as Json, json};

pub(crate) fn node(v: Json) -> Node {
    serde_json::from_value(v).unwrap()
}
pub(crate) fn rect(id: &str, width: Json) -> Json {
    json!({"type":"rect", "id":id,"name":"TEST DATA shape","width":width,"height":10})
}
pub(crate) fn frame(id: &str, children: Json) -> Json {
    json!({"type":"frame","id":id,"name":"TEST DATA frame","width":100,"height":100,"children":children})
}
pub(crate) fn instance(id: &str, component: &str, extra: Json) -> Node {
    let mut v = json!({"type":"instance","id":id,"name":"TEST DATA instance","componentId":component,"width":"hug","height":"hug"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    node(v)
}
pub(crate) fn doc() -> Document {
    serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA components","createdWith":"tests"},
        "components":{"card":{"name":"TEST DATA Card","root":frame("root",json!([rect("label",json!(20)),
            {"type":"frame","id":"slot","name":"TEST DATA slot","width":"fill","height":"hug","slot":{"suggested":["badge"]}}]))},
        "badge":{"name":"TEST DATA Badge","root":frame("badgeRoot",json!([rect("dot",json!(4))]))}},
        "pages":[{"id":"page","name":"TEST DATA page"}]})).unwrap()
}
pub(crate) struct Empty;
impl LibrarySource for Empty {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}
fn resolve(d: &Document, n: &Node) -> Arc<ResolvedNode> {
    Resolver::new().resolve_node(d, "/repo/main.vellum", n, &mut Libraries::new(&Empty))
}
fn width(n: &ResolvedNode) -> f64 {
    match n.value.node.width() {
        vellum_types::Size::Fixed(vellum_types::Value::Literal(v)) => *v,
        other => panic!("expected width, got {other:?}"),
    }
}

#[test]
fn instance_tree_and_per_instance_cache_addresses() {
    let d = doc();
    let mut r = Resolver::new();
    let mut libs = Libraries::new(&Empty);
    let a = instance(
        "a",
        "card",
        json!({"overrides":{"root/label":{"props":{"width":33}}}}),
    );
    let b = instance("b", "card", json!({}));
    let x = r.resolve_nodes(&d, "/repo/main.vellum", &[a, b], &mut libs);
    assert_eq!(width(&x[0].children[0]), 33.);
    assert_eq!(width(&x[1].children[0]), 20.);
    assert_ne!(x[0].children[0].address, x[1].children[0].address);
    assert_eq!(x[0].value.node.kind(), vellum_types::NodeKind::Frame);
    assert_eq!(x[0].value.node.id().as_str(), "a");
    assert!(x[0].all_problems().is_empty());
}
#[test]
fn all_override_forms_and_variant_precedence() {
    let mut d = doc();
    d.components.get_mut(&ComponentId::new("card").unwrap()).unwrap().variants=serde_json::from_value(json!([
      {"properties":{"State":"active"},"overrides":{"root/label":{"replace":rect("replacement",json!(70))}}}])).unwrap();
    let n = instance(
        "a",
        "card",
        json!({"variant":{"State":"active"},"overrides":{"root/label":{"props":{"width":42}},"root/slot":{"children":[rect("owned",json!(9))]}}}),
    );
    let x = resolve(&d, &n);
    assert_eq!(width(&x.children[0]), 42.);
    assert_eq!(x.children[0].value.node.id().as_str(), "label");
    assert_eq!(x.children[1].children[0].value.node.id().as_str(), "owned");
    d.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()[0] = node(rect("label", json!(999)));
    assert_eq!(width(&resolve(&d, &n).children[0]), 42.);
}
#[test]
fn slots_are_per_instance_suggestions_are_advisory() {
    let d = doc();
    let n = instance(
        "a",
        "card",
        json!({"slotContent":{"slot":[rect("custom",json!(8)),serde_json::to_value(instance("inner","badge",json!({}))).unwrap()]}}),
    );
    let x = resolve(&d, &n);
    let slot = &x.children[1];
    assert_eq!(slot.children.len(), 2);
    assert_eq!(slot.slot.as_ref().unwrap().suggested[0].as_str(), "badge");
    assert_eq!(slot.children[1].children[0].value.node.id().as_str(), "dot");
    assert!(
        resolve(&d, &instance("b", "card", json!({}))).children[1]
            .children
            .is_empty()
    );
    assert!(x.all_problems().is_empty());
}
#[test]
fn bad_paths_patch_types_variants_and_cycles_are_reported() {
    let mut d = doc();
    for extra in [
        json!({"overrides":{"root/nope":{"props":{"width":3}}}}),
        json!({"overrides":{"root/label":{"props":{"content":[]}}}}),
        json!({"variant":{"Unknown":"x"}}),
        json!({"slotContent":{"label":[]}}),
    ] {
        assert!(
            !resolve(&d, &instance("a", "card", extra))
                .all_problems()
                .is_empty()
        );
    }
    d.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()
        .push(instance("recursive", "card", json!({})));
    let x = resolve(&d, &instance("a", "card", json!({})));
    assert!(!x.all_problems().is_empty());
}
#[test]
fn props_defaults_types_binding_text_and_override_precedence() {
    let mut d = doc();
    let c = d
        .components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap();
    c.root.children_mut().unwrap().push(node(json!({"type":"text","id":"text","name":"TEST DATA text","width":10,"height":10,"content":[{"text":"old","font":"sans-serif","size":12}]})));
    c.props=serde_json::from_value(json!({
      "size":{"type":"number","default":25,"bindings":[{"node":"root/label","property":"width"}]},
      "shown":{"type":"boolean","default":false,"bindings":[{"node":"root/label","property":"visible"}]},
      "label":{"type":"text","default":"Default","bindings":[{"node":"root/text","property":"content"}]}})).unwrap();
    let x = resolve(&d, &instance("a", "card", json!({})));
    assert_eq!(width(&x.children[0]), 25.);
    assert!(!x.children[0].value.visible);
    let Node::Text(t) = &x.children[2].value.node else {
        panic!()
    };
    assert_eq!(t.content[0].text, "Default");
    let n = instance(
        "a",
        "card",
        json!({"props":{"size":35,"shown":true,"label":"Instance"},"overrides":{"root/label":{"props":{"width":45}}}}),
    );
    let x = resolve(&d, &n);
    assert_eq!(width(&x.children[0]), 45.);
    assert!(x.children[0].value.visible);
    let Node::Text(t) = &x.children[2].value.node else {
        panic!()
    };
    assert_eq!(t.content[0].text, "Instance");
    assert!(
        !resolve(
            &d,
            &instance("a", "card", json!({"props":{"size":"wrong"}}))
        )
        .all_problems()
        .is_empty()
    );
}
#[test]
fn variable_override_type_is_checked_after_patch() {
    let mut d = doc();
    d.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA size","type":"number","values":[{"value":66}]}}),
    )
    .unwrap();
    let x = resolve(
        &d,
        &instance(
            "a",
            "card",
            json!({"overrides":{"root/label":{"props":{"width":{"$var":"size"}}}}}),
        ),
    );
    assert_eq!(width(&x.children[0]), 66.);
    assert!(x.all_problems().is_empty());
    let x = resolve(
        &d,
        &instance(
            "a",
            "card",
            json!({"overrides":{"root/label":{"props":{"visible":{"$var":"size"}}}}}),
        ),
    );
    assert!(!x.all_problems().is_empty());
}
#[test]
fn cached_matches_fresh_after_component_and_slot_edits() {
    let mut d = doc();
    let n = instance("a", "card", json!({}));
    let mut r = Resolver::new();
    let mut libs = Libraries::new(&Empty);
    let before = r.resolve_node(&d, "/repo/main.vellum", &n, &mut libs);
    let same = r.resolve_node(&d, "/repo/main.vellum", &n, &mut libs);
    assert!(Arc::ptr_eq(&before, &same));
    d.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()[0] = node(rect("label", json!(55)));
    let after = r.resolve_node(&d, "/repo/main.vellum", &n, &mut libs);
    assert_eq!(after, resolve(&d, &n));
    assert_eq!(width(&after.children[0]), 55.);
    assert!(Arc::ptr_eq(&before.children[1], &after.children[1]));
}

#[test]
fn finite_nested_slot_instances_can_reuse_the_same_component() {
    let d = doc();
    let inner = instance("inner", "card", json!({}));
    let outer = instance("outer", "card", json!({"slotContent":{"slot":[inner]}}));
    let tree = resolve(&d, &outer);
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    assert_eq!(tree.children[1].children[0].children.len(), 2);
}

#[test]
fn transactions_refuse_wrong_variable_types_in_props_overrides() {
    let mut d = doc();
    d.variables = serde_json::from_value(
        json!({"size":{"name":"TEST DATA size","type":"number","values":[{"value":66}]}}),
    )
    .unwrap();
    d.pages[0].children.push(instance("a", "card", json!({})));
    let patch = serde_json::from_value(
        json!({"overrides":{"root/label":{"props":{"visible":{"$var":"size"}}}}}),
    )
    .unwrap();
    let result = crate::ops::tests::common::planned(
        &d,
        vec![crate::ops::Op::Update {
            id: vellum_types::NodeId::new("a").unwrap(),
            patch,
        }],
    );
    assert!(
        result.is_err(),
        "props override bypassed variable type validation"
    );
}

#[test]
fn detach_uses_prop_bindings_and_keeps_nested_component_links() {
    let mut d = doc();
    d.components.get_mut(&ComponentId::new("card").unwrap()).unwrap().props=serde_json::from_value(json!({"size":{"type":"number","default":37,"bindings":[{"node":"root/label","property":"width"}]}})).unwrap();
    let n = instance(
        "a",
        "card",
        json!({"slotContent":{"slot":[instance("inner","badge",json!({}))]}}),
    );
    d.pages[0].children.push(n);
    let plan = crate::ops::tests::common::planned(
        &d,
        vec![crate::ops::Op::Detach {
            id: vellum_types::NodeId::new("a").unwrap(),
        }],
    )
    .unwrap();
    assert_eq!(
        plan.document.pages[0].children[0].children()[0].width(),
        &vellum_types::Size::Fixed(37.0.into())
    );
    assert!(matches!(
        plan.document.pages[0].children[0].children()[1].children()[0],
        Node::Instance(_)
    ));
}

#[test]
fn unselected_variant_props_are_checked_when_component_changes() {
    let d = doc();
    let mut component = d.components[&ComponentId::new("card").unwrap()].clone();
    component.variants=serde_json::from_value(json!([{"properties":{"State":"bad"},"overrides":{"root/label":{"props":{"width":"wrong type"}}}}])).unwrap();
    assert!(
        crate::ops::tests::common::planned(
            &d,
            vec![crate::ops::Op::SetComponent {
                id: ComponentId::new("card").unwrap(),
                component: Some(Box::new(component))
            }]
        )
        .is_err()
    );
}
#[test]
fn nested_instance_override_paths_keep_the_nested_link() {
    let mut d = doc();
    d.components
        .get_mut(&ComponentId::new("card").unwrap())
        .unwrap()
        .root
        .children_mut()
        .unwrap()
        .push(instance("inner", "badge", json!({})));
    let n = instance(
        "a",
        "card",
        json!({"overrides":{"root/inner/badgeRoot/dot":{"props":{"width":87}}}}),
    );
    let tree = resolve(&d, &n);
    assert!(tree.all_problems().is_empty(), "{:?}", tree.all_problems());
    assert_eq!(width(&tree.children[2].children[0]), 87.);
    assert!(tree.children[2].instance.is_some());
    d.pages[0].children.push(n);
    let plan = crate::ops::tests::common::planned(
        &d,
        vec![crate::ops::Op::Detach {
            id: vellum_types::NodeId::new("a").unwrap(),
        }],
    )
    .unwrap();
    assert!(matches!(
        plan.document.pages[0].children[0].children()[2],
        Node::Instance(_)
    ));
}

proptest::proptest! {
  #![proptest_config(proptest::test_runner::Config::with_cases(128))]
  #[test]
  fn incremental_resolution_equals_fresh_after_mixed_edits(steps in proptest::collection::vec(proptest::prelude::any::<u8>(),1..40)) {
    let mut d=doc();let mut n=instance("a","card",json!({}));let mut cached=Resolver::new();
    for step in steps {
      match step%5 {
        0=>{ d.components.get_mut(&ComponentId::new("card").unwrap()).unwrap().root.children_mut().unwrap()[0]=node(rect("label",json!(step as f64))); },
        1=>{ let Node::Instance(i)=&mut n else{unreachable!()};i.overrides=serde_json::from_value(json!({"root/label":{"props":{"width":step as f64}}})).unwrap(); },
        2=>{ let Node::Instance(i)=&mut n else{unreachable!()};i.slot_content=serde_json::from_value(json!({"slot":[rect("payload",json!(step as f64))]})).unwrap(); },
        3=>{ let Node::Instance(i)=&mut n else{unreachable!()};i.overrides.clear();i.slot_content.clear(); },
        _=>{ d.variables=serde_json::from_value(json!({"size":{"name":"TEST DATA size","type":"number","values":[{"value":step as f64}]}})).unwrap();d.components.get_mut(&ComponentId::new("card").unwrap()).unwrap().root.children_mut().unwrap()[0]=node(rect("label",json!({"$var":"size"}))); },
      }
      let before=d.clone();let a=cached.resolve_node(&d,"/repo/main.vellum",&n,&mut Libraries::new(&Empty));
      let b=resolve(&d,&n);proptest::prop_assert_eq!(a,b);proptest::prop_assert_eq!(&d,&before);
    }
  }
}
