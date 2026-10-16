//! Prompt 12 acceptance cases. TEST DATA only.
use super::common::*;
use crate::ops::{self, Alignment, Axis, Inputs, Op, OpErrorKind};
use crate::ports::{Path, PathOps, Rect, StrokeStyle};
use serde_json::json;
use std::collections::BTreeMap;
use vellum_types::{BoolOp, ComponentId, Node, NodeIdPath, NodeKind, Point, Size, Value};

fn component_id(s: &str) -> ComponentId {
    ComponentId::new(s).unwrap()
}
fn path(s: &str) -> NodeIdPath {
    s.parse().unwrap()
}
fn inverse(doc: &vellum_types::Document, plan: &ops::Plan) {
    let mut after = plan.document.clone();
    ops::apply_edits(&mut after, &plan.inverse, alice()).unwrap();
    assert_eq!(&after, doc);
    let mut replay = doc.clone();
    ops::apply_edits(&mut replay, &plan.edits, alice()).unwrap();
    assert_eq!(replay, plan.document);
    assert!(
        ops::check_document(&replay).is_empty(),
        "{:?}",
        ops::check_document(&replay)
    );
}
fn instance() -> Node {
    shape(
        "instance",
        "inst000001",
        json!({"componentId":"comp000001"}),
    )
}
fn with_instance() -> vellum_types::Document {
    let mut doc = document();
    doc.pages[0].children.push(instance());
    doc
}

#[test]
fn component_creation_preserves_appearance_and_is_invertible() {
    for target in ["frame00001", "rect000003"] {
        let doc = document();
        let p = planned(
            &doc,
            vec![Op::CreateComponent {
                id: id(target),
                component_id: component_id("comp000002"),
            }],
        )
        .unwrap();
        let Node::Instance(n) = find(&p.document, target).unwrap() else {
            panic!()
        };
        assert_eq!(n.component_id, component_id("comp000002"));
        assert_eq!(n.position, *find(&doc, target).unwrap().position());
        assert_eq!(p.document.components.len(), 2);
        assert!(matches!(
            p.document.components[&component_id("comp000002")].root,
            Node::Frame(_)
        ));
        inverse(&doc, &p);
    }
}
#[test]
fn toggle_to_component_and_back_is_one_reversible_transaction() {
    let doc = document();
    let p = planned(
        &doc,
        vec![
            Op::ToggleComponent {
                id: id("frame00001"),
                component_id: component_id("comp000002"),
            },
            Op::ToggleComponent {
                id: id("frame00001"),
                component_id: component_id("comp000002"),
            },
        ],
    )
    .unwrap();
    assert!(matches!(
        find(&p.document, "frame00001"),
        Some(Node::Frame(_))
    ));
    assert!(
        !p.document
            .components
            .contains_key(&component_id("comp000002"))
    );
    inverse(&doc, &p);
}
#[test]
fn detach_keeps_nested_instances_variables_variant_override_and_slot_content() {
    let mut doc = with_instance();
    let root = &mut doc
        .components
        .get_mut(&component_id("comp000001"))
        .unwrap()
        .root;
    root.children_mut().unwrap().push(shape(
        "instance",
        "nested0001",
        json!({"componentId":"comp000002"}),
    ));
    root.children_mut().unwrap().push(shape(
        "frame",
        "slot000001",
        json!({"slot":{"suggested":[]}}),
    ));
    doc.components.insert(
        component_id("comp000002"),
        vellum_types::Component {
            name: "TEST DATA nested".into(),
            props: BTreeMap::new(),
            variants: vec![],
            root: shape("frame", "nestedroot", json!({})),
        },
    );
    let Node::Instance(n) = doc.pages[0].children.last_mut().unwrap() else {
        panic!()
    };
    n.overrides.insert(
        path("croot00001/ctext00001"),
        vellum_types::Override::Props(patch(
            json!({"fills":[{"type":"solid","color":{"$var":"color00001"}}]}),
        )),
    );
    n.slot_content.insert(
        id("slot000001"),
        vec![shape("rect", "slotchild1", json!({}))],
    );
    let p = planned(
        &doc,
        vec![Op::Detach {
            id: id("inst000001"),
        }],
    )
    .unwrap();
    let root = find(&p.document, "inst000001").unwrap();
    assert!(matches!(root.children()[1], Node::Instance(_)));
    assert_eq!(
        root.children()[0].fills()[0],
        serde_json::from_value(json!({"type":"solid","color":{"$var":"color00001"}})).unwrap()
    );
    assert_eq!(root.children()[2].children().len(), 1);
    assert_eq!(p.document.components, doc.components);
    inverse(&doc, &p);
}
#[test]
fn replacement_inside_instance_does_not_change_the_definition() {
    let doc = with_instance();
    let p = planned(
        &doc,
        vec![Op::ReplaceInInstance {
            id: id("inst000001"),
            path: path("croot00001/ctext00001"),
            node: Box::new(shape("rect", "replacement", json!({}))),
        }],
    )
    .unwrap();
    assert_eq!(p.document.components, doc.components);
    let Node::Instance(n) = find(&p.document, "inst000001").unwrap() else {
        panic!()
    };
    assert!(matches!(
        n.overrides[&path("croot00001/ctext00001")],
        vellum_types::Override::Replace(_)
    ));
    inverse(&doc, &p);
}
#[test]
fn slots_require_empty_nonroot_component_frames_and_suggestions_do_not_restrict() {
    let mut doc = document();
    doc.components
        .get_mut(&component_id("comp000001"))
        .unwrap()
        .root
        .children_mut()
        .unwrap()
        .push(shape("frame", "slot000001", json!({})));
    let p = planned(
        &doc,
        vec![Op::MakeSlot {
            id: id("slot000001"),
            suggested: vec![component_id("comp000001")],
        }],
    )
    .unwrap();
    let Node::Frame(n) = find(&p.document, "slot000001").unwrap() else {
        panic!()
    };
    assert_eq!(
        n.slot.as_ref().unwrap().suggested,
        vec![component_id("comp000001")]
    );
    inverse(&doc, &p);
    for target in ["frame00001", "croot00001", "ctext00001"] {
        assert!(
            planned(
                &doc,
                vec![Op::MakeSlot {
                    id: id(target),
                    suggested: vec![]
                }]
            )
            .is_err()
        );
    }
    assert!(
        planned(
            &doc,
            vec![Op::MakeSlot {
                id: id("slot000001"),
                suggested: vec![component_id("missing001")]
            }]
        )
        .is_err()
    );
}
#[test]
fn inherited_structure_cannot_be_reparented_into_an_instance() {
    let doc = with_instance();
    assert!(
        planned(
            &doc,
            vec![Op::Move {
                id: id("ctext00001"),
                to: in_node("inst000001", 0)
            }]
        )
        .is_err()
    );
    assert!(
        planned(
            &doc,
            vec![Op::Insert {
                to: in_node("inst000001", 0),
                node: Box::new(shape("rect", "newrect001", json!({})))
            }]
        )
        .is_err()
    );
}
#[test]
fn component_errors_are_atomic_and_located() {
    let doc = with_instance();
    let invalid = vec![
        Op::CreateComponent {
            id: id("missing001"),
            component_id: component_id("comp000002"),
        },
        Op::CreateComponent {
            id: id("frame00001"),
            component_id: component_id("comp000001"),
        },
        Op::CreateComponent {
            id: id("inst000001"),
            component_id: component_id("comp000002"),
        },
        Op::Detach {
            id: id("rect000003"),
        },
        Op::ReplaceInInstance {
            id: id("inst000001"),
            path: path("croot00001/missing001"),
            node: Box::new(shape("rect", "newrect001", json!({}))),
        },
        Op::ReplaceInInstance {
            id: id("inst000001"),
            path: path("croot00001"),
            node: Box::new(shape("rect", "newrect001", json!({}))),
        },
    ];
    for op in invalid {
        let mut store = Store::new(doc.clone());
        let error = ops::apply(
            &mut store,
            &mut Ids::default(),
            tx(vec![
                Op::Delete {
                    id: id("rect000003"),
                },
                op,
            ]),
        )
        .unwrap_err();
        assert_eq!(error.op_index, 1);
        assert!(!error.path.is_empty());
        assert_eq!(store.doc, doc);
        assert_eq!(store.applied, 0);
    }
}

#[test]
fn scale_fixture_scales_all_lengths_and_preserves_ratios_and_links() {
    let doc: vellum_types::Document =
        serde_json::from_str(include_str!("../../../../../fixtures/ops/scale.vellum")).unwrap();
    let mut inputs = Inputs::default();
    inputs
        .bounds
        .insert(id("text000001"), Rect::new(10., 20., 80., 30.));
    inputs.numbers.insert(var_id("number0001"), 16.);
    let p = ops::plan_with_inputs(
        &doc,
        &tx(vec![Op::Scale {
            id: id("frame00001"),
            factor: 2.,
            anchor: Point { x: 0.5, y: 0.5 },
        }]),
        &mut Ids::default(),
        &inputs,
    )
    .unwrap();
    let root = find(&p.document, "frame00001").unwrap();
    assert_eq!(root.width(), &Size::Fixed(Value::Literal(200.)));
    assert_eq!(
        root.position(),
        &vellum_types::Position::Absolute(Point { x: -40., y: -30. })
    );
    let data = serde_json::to_value(root).unwrap();
    assert_eq!(data["gap"].as_f64(), Some(32.));
    assert_eq!(data["padding"], json!([4., 8., 12., 16.]));
    assert_eq!(data["cornerRadius"], json!([2., 4., 6., 8.]));
    assert_eq!(data["stroke"]["width"], json!([2., 4., 6., 8.]));
    assert_eq!(data["stroke"]["dash"], json!([4., 6.]));
    assert_eq!(data["effects"][0]["blur"].as_f64(), Some(8.));
    assert_eq!(data["effects"][0]["x"].as_f64(), Some(4.));
    let text = find(&p.document, "text000001").unwrap();
    let data = serde_json::to_value(text).unwrap();
    assert_eq!(data["content"][0]["size"].as_f64(), Some(32.));
    assert_eq!(data["content"][0]["lineHeight"].as_f64(), Some(40.));
    assert_eq!(data["content"][0]["letterSpacing"].as_f64(), Some(2.));
    assert_eq!(data["opacity"], 0.5);
    assert_eq!(data["width"].as_f64(), Some(160.));
    assert_eq!(data["height"].as_f64(), Some(60.));
    assert_eq!(data["fills"][0]["color"]["$var"], "color00001");
    inverse(&doc, &p);
}
#[test]
fn scaling_an_instance_writes_overrides_and_never_edits_the_definition() {
    let doc = with_instance();
    let mut inputs = Inputs::default();
    inputs
        .bounds
        .insert(id("croot00001"), Rect::new(0., 0., 10., 10.));
    inputs
        .bounds
        .insert(id("ctext00001"), Rect::new(0., 0., 10., 10.));
    let p = ops::plan_with_inputs(
        &doc,
        &tx(vec![Op::Scale {
            id: id("inst000001"),
            factor: 2.,
            anchor: Point::default(),
        }]),
        &mut Ids::default(),
        &inputs,
    )
    .unwrap();
    assert_eq!(p.document.components, doc.components);
    let Node::Instance(n) = find(&p.document, "inst000001").unwrap() else {
        panic!()
    };
    let vellum_types::Override::Props(text) = &n.overrides[&path("croot00001/ctext00001")] else {
        panic!()
    };
    assert_eq!(text.properties()["content"][0]["size"].as_f64(), Some(28.));
    inverse(&doc, &p);
}
#[test]
fn invalid_scales_refuse_with_no_commit() {
    for factor in [0., -1., f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(
            planned(
                &document(),
                vec![Op::Scale {
                    id: id("frame00001"),
                    factor,
                    anchor: Point::default()
                }]
            )
            .is_err()
        );
    }
    assert!(
        planned(
            &document(),
            vec![Op::Scale {
                id: id("frame00001"),
                factor: 2.,
                anchor: Point { x: 2., y: 0. }
            }]
        )
        .is_err()
    );
    assert!(
        planned(
            &document(),
            vec![Op::Scale {
                id: id("text000001"),
                factor: 2.,
                anchor: Point::default()
            }]
        )
        .is_err()
    );
}
#[test]
fn align_distribute_and_flip_preserve_order_and_have_exact_inverses() {
    let mut doc = document();
    doc.pages[0]
        .children
        .push(shape("rect", "rect000005", at(80., 70.)));
    let ids = vec![id("rect000001"), id("rect000002")];
    for alignment in [Alignment::Start, Alignment::Center, Alignment::End] {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let p = planned(
                &doc,
                vec![Op::Align {
                    ids: ids.clone(),
                    axis,
                    alignment,
                }],
            )
            .unwrap();
            inverse(&doc, &p);
        }
    }
    let p = planned(
        &doc,
        vec![
            Op::Flip {
                id: id("frame00001"),
                axis: Axis::Horizontal,
            },
            Op::Flip {
                id: id("frame00001"),
                axis: Axis::Vertical,
            },
        ],
    )
    .unwrap();
    assert!(find(&p.document, "frame00001").unwrap().flip_x());
    assert!(find(&p.document, "frame00001").unwrap().flip_y());
    inverse(&doc, &p);
    let mut doc = document();
    doc.pages[0].children[0].children_mut().unwrap().push(shape(
        "rect",
        "rect000005",
        at(80., 70.),
    ));
    let p = planned(
        &doc,
        vec![Op::Distribute {
            ids: vec![id("rect000005"), id("rect000001"), id("rect000002")],
            axis: Axis::Horizontal,
        }],
    )
    .unwrap();
    assert_eq!(position(&p.document, "rect000002")["x"], 45.);
    inverse(&doc, &p);
    for op in [
        Op::Align {
            ids: vec![],
            axis: Axis::Horizontal,
            alignment: Alignment::Start,
        },
        Op::Distribute {
            ids: ids.clone(),
            axis: Axis::Vertical,
        },
        Op::Align {
            ids: vec![id("rect000001"), id("rect000003")],
            axis: Axis::Horizontal,
            alignment: Alignment::End,
        },
        Op::Align {
            ids: vec![id("rect000001"), id("rect000001")],
            axis: Axis::Horizontal,
            alignment: Alignment::Start,
        },
    ] {
        assert!(planned(&doc, vec![op]).is_err());
    }
}
struct Geometry;
impl PathOps for Geometry {
    fn boolean(&self, a: &Path, b: &Path, op: BoolOp) -> Path {
        // TEST DATA: visibly distinct outputs prove the enum and input order reach the port.
        assert!(!a.is_empty());
        assert!(!b.is_empty());
        let width = match op {
            BoolOp::Union => 40.,
            BoolOp::Subtract => 30.,
            BoolOp::Intersect => 20.,
            BoolOp::Exclude => 10.,
        };
        Path::rect(0., 0., width, 10.)
    }
    fn bounds(&self, p: &Path, _: Option<&StrokeStyle>) -> Option<Rect> {
        if p.is_empty() {
            return None;
        }
        let crate::ports::PathSegment::LineTo(point) = p.segments[1] else {
            panic!()
        };
        let crate::ports::PathSegment::MoveTo(first) = p.segments[0] else {
            panic!()
        };
        let crate::ports::PathSegment::LineTo(bottom) = p.segments[2] else {
            panic!()
        };
        Some(Rect::new(
            first.x,
            first.y,
            point.x - first.x,
            bottom.y - first.y,
        ))
    }
    fn outline_stroke(&self, p: &Path, _: &StrokeStyle) -> Result<Path, String> {
        assert!(!p.is_empty());
        Ok(Path::rect(-1., -1., 12., 12.))
    }
}
#[test]
fn booleans_delegate_all_modes_and_keep_editable_children() {
    let doc = document();
    let geometry = Geometry;
    let mut inputs = Inputs {
        path_ops: Some(&geometry),
        ..Inputs::default()
    };
    inputs
        .paths
        .insert(id("rect000001"), Path::rect(10., 10., 10., 10.));
    inputs
        .paths
        .insert(id("rect000002"), Path::rect(30., 30., 10., 10.));
    for mode in [
        BoolOp::Union,
        BoolOp::Subtract,
        BoolOp::Intersect,
        BoolOp::Exclude,
    ] {
        let p = ops::plan_with_inputs(
            &doc,
            &tx(vec![Op::Boolean {
                ids: vec![id("rect000001"), id("rect000002")],
                id: id("boolean002"),
                op: mode,
            }]),
            &mut Ids::default(),
            &inputs,
        )
        .unwrap();
        let Node::Boolean(n) = find(&p.document, "boolean002").unwrap() else {
            panic!()
        };
        assert_eq!(n.op, mode);
        assert_eq!(n.children.len(), 2);
        let expected = match mode {
            BoolOp::Union => 40.,
            BoolOp::Subtract => 30.,
            BoolOp::Intersect => 20.,
            BoolOp::Exclude => 10.,
        };
        assert_eq!(n.width, Size::Fixed(Value::Literal(expected)));
        inverse(&doc, &p);
    }
    assert!(
        planned(
            &doc,
            vec![Op::Boolean {
                ids: vec![id("rect000001"), id("rect000002")],
                id: id("boolean002"),
                op: BoolOp::Union
            }]
        )
        .is_err()
    );
}
#[test]
fn outline_stroke_delegates_and_replaces_with_painted_vector() {
    let mut doc = document();
    doc.pages[0].children[1] = shape(
        "rect",
        "rect000003",
        json!({"stroke":{"paints":[{"type":"solid","color":"#3366FF"}],"width":2}}),
    );
    let geometry = Geometry;
    let mut inputs = Inputs {
        path_ops: Some(&geometry),
        ..Inputs::default()
    };
    inputs
        .paths
        .insert(id("rect000003"), Path::rect(0., 0., 10., 10.));
    let p = ops::plan_with_inputs(
        &doc,
        &tx(vec![Op::OutlineStroke {
            id: id("rect000003"),
        }]),
        &mut Ids::default(),
        &inputs,
    )
    .unwrap();
    let Node::Vector(n) = find(&p.document, "rect000003").unwrap() else {
        panic!()
    };
    assert!(n.path.contains("M -1 -1"));
    assert_eq!(n.fills, doc.pages[0].children[1].stroke().unwrap().paints);
    assert!(n.stroke.is_none());
    assert_eq!(n.width, Size::Fixed(Value::Literal(12.)));
    assert_eq!(
        n.view_box,
        vellum_types::ViewBox {
            x: -1.,
            y: -1.,
            width: 12.,
            height: 12.
        }
    );
    inverse(&doc, &p);
    assert!(
        ops::plan_with_inputs(
            &doc,
            &tx(vec![Op::OutlineStroke {
                id: id("rect000001")
            }]),
            &mut Ids::default(),
            &inputs
        )
        .is_err()
    );
}
#[test]
fn convert_script_requires_a_script() {
    let error = refused(vec![Op::ConvertScript {
        id: id("frame00001"),
    }]);
    assert_eq!(
        error.kind,
        OpErrorKind::WrongKind {
            expected: NodeKind::Script,
            found: NodeKind::Frame
        }
    );
}

#[test]
fn instance_scaling_repeats_without_doubling_root_and_scales_replacements_once() {
    let mut doc = with_instance();
    let c = doc.components.get_mut(&component_id("comp000001")).unwrap();
    c.root = shape(
        "frame",
        "croot00001",
        json!({"width":10,"height":10,"children":[{"type":"rect","id":"crect00001","name":"TEST DATA","width":4,"height":4}]}),
    );
    let p = planned(
        &doc,
        vec![
            Op::Scale {
                id: id("inst000001"),
                factor: 2.,
                anchor: Point::default(),
            },
            Op::Scale {
                id: id("inst000001"),
                factor: 2.,
                anchor: Point::default(),
            },
        ],
    )
    .unwrap();
    let Node::Instance(n) = find(&p.document, "inst000001").unwrap() else {
        panic!()
    };
    let vellum_types::Override::Props(root) = &n.overrides[&path("croot00001")] else {
        panic!()
    };
    assert_eq!(root.properties()["width"].as_f64(), Some(40.));
    let vellum_types::Override::Props(child) = &n.overrides[&path("croot00001/crect00001")] else {
        panic!()
    };
    assert_eq!(child.properties()["width"].as_f64(), Some(16.));
    inverse(&doc, &p);
    let p=planned(&doc,vec![Op::ReplaceInInstance{id:id("inst000001"),path:path("croot00001/crect00001"),node:Box::new(shape("frame","replace001",json!({"children":[{"type":"rect","id":"replacekid","name":"TEST DATA","width":3,"height":3}]})))},Op::Scale{id:id("inst000001"),factor:2.,anchor:Point::default()},Op::Detach{id:id("inst000001")}]).unwrap();
    assert_eq!(
        find(&p.document, "inst000001").unwrap().children()[0].width(),
        &Size::Fixed(Value::Literal(20.))
    );
    assert_eq!(
        find(&p.document, "inst000001").unwrap().children()[0].children()[0].width(),
        &Size::Fixed(Value::Literal(6.))
    );
    inverse(&doc, &p);
}
#[test]
fn update_insert_and_replacement_cannot_bypass_component_or_slot_checks() {
    let doc = with_instance();
    for op in [
        Op::Update {
            id: id("inst000001"),
            patch: patch(json!({"componentId":"missing001"})),
        },
        Op::Update {
            id: id("frame00001"),
            patch: patch(json!({"slot":{"suggested":[]}})),
        },
        Op::ReplaceInInstance {
            id: id("inst000001"),
            path: path("croot00001/ctext00001"),
            node: Box::new(shape("rect", "rect000003", json!({}))),
        },
        Op::ReplaceInInstance {
            id: id("inst000001"),
            path: path("croot00001/ctext00001"),
            node: Box::new(shape(
                "frame",
                "replace001",
                json!({"children":[{"type":"rect","id":"same000001","name":"TEST DATA","width":1,"height":1},{"type":"rect","id":"same000001","name":"TEST DATA","width":1,"height":1}]}),
            )),
        },
    ] {
        assert!(planned(&doc, vec![op]).is_err());
    }
}
#[test]
fn nonframe_component_keeps_style_and_transform_once() {
    let mut doc = document();
    doc.pages[0].children[1] = shape(
        "rect",
        "rect000003",
        json!({"opacity":0.5,"rotation":45,"effects":[{"type":"layer-blur","radius":4}]}),
    );
    let p = planned(
        &doc,
        vec![
            Op::CreateComponent {
                id: id("rect000003"),
                component_id: component_id("comp000002"),
            },
            Op::Detach {
                id: id("rect000003"),
            },
        ],
    )
    .unwrap();
    let wrapper = find(&p.document, "rect000003").unwrap();
    assert_eq!(wrapper.opacity(), &Value::Literal(1.));
    assert_eq!(wrapper.rotation(), &Value::Literal(0.));
    assert!(wrapper.effects().is_empty());
    let child = &wrapper.children()[0];
    assert_eq!(child.opacity(), &Value::Literal(0.5));
    assert_eq!(child.rotation(), &Value::Literal(45.));
    assert_eq!(child.effects().len(), 1);
    inverse(&doc, &p);
}

#[test]
fn measured_boxes_do_not_override_new_fixed_sizes_in_a_batch() {
    let doc: vellum_types::Document =
        serde_json::from_str(include_str!("../../../../../fixtures/ops/scale.vellum")).unwrap();
    let mut inputs = Inputs::default();
    inputs
        .bounds
        .insert(id("text000001"), Rect::new(10., 20., 80., 30.));
    inputs.numbers.insert(var_id("number0001"), 16.);
    let p = ops::plan_with_inputs(
        &doc,
        &tx(vec![
            Op::Scale {
                id: id("frame00001"),
                factor: 2.,
                anchor: Point::default(),
            },
            Op::Scale {
                id: id("frame00001"),
                factor: 2.,
                anchor: Point::default(),
            },
        ]),
        &mut Ids::default(),
        &inputs,
    )
    .unwrap();
    assert_eq!(
        find(&p.document, "text000001").unwrap().width(),
        &Size::Fixed(Value::Literal(320.))
    );
    inverse(&doc, &p);
}
#[test]
fn slots_accept_content_outside_the_suggestions_and_can_be_undone() {
    let doc: vellum_types::Document =
        serde_json::from_str(include_str!("../../../../../fixtures/ops/slots.vellum")).unwrap();
    let p = planned(
        &doc,
        vec![Op::Detach {
            id: id("inst000001"),
        }],
    )
    .unwrap();
    let slot = &find(&p.document, "inst000001").unwrap().children()[0];
    assert_eq!(slot.children().len(), 1);
    assert!(matches!(slot.children()[0], Node::Rect(_)));
    inverse(&doc, &p);
}
#[test]
fn variants_and_children_overrides_survive_scaling_and_detach() {
    let mut doc = with_instance();
    let c = doc.components.get_mut(&component_id("comp000001")).unwrap();
    c.root = shape(
        "frame",
        "croot00001",
        json!({"children":[{"type":"frame","id":"cframe0001","name":"TEST DATA","width":10,"height":10}]}),
    );
    c.variants=vec![serde_json::from_value(json!({"properties":{"State":"large"},"overrides":{"croot00001/cframe0001":{"props":{"gap":3}}}})).unwrap()];
    let Node::Instance(n) = doc.pages[0].children.last_mut().unwrap() else {
        panic!()
    };
    n.variant.insert("State".into(), "large".into());
    n.overrides.insert(
        path("croot00001/cframe0001"),
        vellum_types::Override::Children(vec![shape(
            "rect",
            "ownrect001",
            json!({"width":4,"height":4}),
        )]),
    );
    let p = planned(
        &doc,
        vec![
            Op::Scale {
                id: id("inst000001"),
                factor: 2.,
                anchor: Point::default(),
            },
            Op::Detach {
                id: id("inst000001"),
            },
        ],
    )
    .unwrap();
    let child = &find(&p.document, "inst000001").unwrap().children()[0];
    let data = serde_json::to_value(child).unwrap();
    assert_eq!(data["gap"].as_f64(), Some(6.));
    assert_eq!(
        child.children()[0].width(),
        &Size::Fixed(Value::Literal(8.))
    );
    inverse(&doc, &p);
}
#[test]
fn nested_instance_scale_keeps_links_and_scales_the_nested_text() {
    let mut doc = with_instance();
    let root = shape(
        "frame",
        "nestedroot",
        json!({"children":[{"type":"text","id":"nestedtext","name":"TEST DATA","width":10,"height":10,"content":[{"text":"TEST DATA","font":"Inter","size":12}]}]}),
    );
    doc.components.insert(
        component_id("comp000002"),
        vellum_types::Component {
            name: "TEST DATA".into(),
            props: BTreeMap::new(),
            variants: vec![],
            root,
        },
    );
    doc.components
        .get_mut(&component_id("comp000001"))
        .unwrap()
        .root = shape(
        "frame",
        "croot00001",
        json!({"children":[{"type":"instance","id":"nested0001","name":"TEST DATA","width":10,"height":10,"componentId":"comp000002"}]}),
    );
    let p = planned(
        &doc,
        vec![
            Op::Scale {
                id: id("inst000001"),
                factor: 2.,
                anchor: Point::default(),
            },
            Op::Detach {
                id: id("inst000001"),
            },
        ],
    )
    .unwrap();
    let Node::Instance(n) = &find(&p.document, "inst000001").unwrap().children()[0] else {
        panic!()
    };
    assert_eq!(n.component_id, component_id("comp000002"));
    let vellum_types::Override::Props(patch) = &n.overrides[&path("nestedroot/nestedtext")] else {
        panic!()
    };
    assert_eq!(patch.properties()["content"][0]["size"].as_f64(), Some(24.));
    inverse(&doc, &p);
}

#[test]
fn rotated_scale_keeps_the_transformed_anchor_fixed() {
    let mut doc = document();
    doc.pages[0].children[1] = shape(
        "rect",
        "rect000003",
        json!({"rotation":90,"position":{"type":"absolute","x":100,"y":100}}),
    );
    let p = planned(
        &doc,
        vec![Op::Scale {
            id: id("rect000003"),
            factor: 2.,
            anchor: Point { x: 0.5, y: 0.5 },
        }],
    )
    .unwrap();
    assert_eq!(
        position(&p.document, "rect000003"),
        json!({"type":"absolute","x":95.,"y":95.})
    );
    inverse(&doc, &p);
    let p = planned(
        &doc,
        vec![Op::Scale {
            id: id("rect000003"),
            factor: 2.,
            anchor: Point { x: 0., y: 0. },
        }],
    )
    .unwrap();
    assert_eq!(
        position(&p.document, "rect000003"),
        json!({"type":"absolute","x":90.,"y":100.})
    );
    inverse(&doc, &p);
}
#[test]
fn path_refusals_leave_a_whole_batch_uncommitted() {
    let doc = document();
    let geometry = Geometry;
    let mut inputs = Inputs {
        path_ops: Some(&geometry),
        ..Inputs::default()
    };
    inputs
        .paths
        .insert(id("rect000001"), Path::rect(10., 10., 10., 10.));
    inputs
        .paths
        .insert(id("rect000002"), Path::rect(30., 30., 10., 10.));
    for op in [
        Op::Boolean {
            ids: vec![id("rect000001")],
            id: id("boolean002"),
            op: BoolOp::Union,
        },
        Op::Boolean {
            ids: vec![id("rect000001"), id("rect000001")],
            id: id("boolean002"),
            op: BoolOp::Union,
        },
        Op::Boolean {
            ids: vec![id("rect000001"), id("rect000003")],
            id: id("boolean002"),
            op: BoolOp::Union,
        },
        Op::Boolean {
            ids: vec![id("rect000001"), id("rect000002")],
            id: id("frame00001"),
            op: BoolOp::Union,
        },
        Op::Boolean {
            ids: vec![id("missing001"), id("rect000002")],
            id: id("boolean002"),
            op: BoolOp::Union,
        },
        Op::OutlineStroke {
            id: id("rect000001"),
        },
    ] {
        let mut store = Store::new(doc.clone());
        let error = ops::apply_with_inputs(
            &mut store,
            &mut Ids::default(),
            tx(vec![
                Op::Flip {
                    id: id("rect000003"),
                    axis: Axis::Horizontal,
                },
                op,
            ]),
            &inputs,
        )
        .unwrap_err();
        assert_eq!(error.op_index, 1);
        assert_eq!(store.doc, doc);
        assert_eq!(store.applied, 0);
    }
}
#[test]
fn generic_edits_cannot_fill_a_main_slot_or_create_recursive_components() {
    let doc: vellum_types::Document =
        serde_json::from_str(include_str!("../../../../../fixtures/ops/slots.vellum")).unwrap();
    assert!(
        planned(
            &doc,
            vec![Op::Insert {
                to: in_node("slot000001", 0),
                node: Box::new(shape("rect", "newrect001", json!({})))
            }]
        )
        .is_err()
    );
    assert!(
        planned(
            &doc,
            vec![Op::Insert {
                to: in_node("croot00001", 0),
                node: Box::new(shape(
                    "instance",
                    "recursive1",
                    json!({"componentId":"comp000001"})
                ))
            }]
        )
        .is_err()
    );
    let mut doc = with_instance();
    doc.pages[0].children.push(shape(
        "instance",
        "inst000002",
        json!({"componentId":"comp000001"}),
    ));
    assert!(
        planned(
            &doc,
            vec![Op::ToggleComponent {
                id: id("inst000001"),
                component_id: component_id("comp000001")
            }]
        )
        .is_err()
    );
}

#[test]
fn instance_overrides_win_over_variant_replacements_at_the_same_path() {
    let mut doc = with_instance();
    let c = doc.components.get_mut(&component_id("comp000001")).unwrap();
    c.root = shape(
        "frame",
        "croot00001",
        json!({"children":[{"type":"rect","id":"crect00001","name":"TEST DATA","width":4,"height":4}]}),
    );
    c.variants=vec![serde_json::from_value(json!({"properties":{"State":"large"},"overrides":{"croot00001/crect00001":{"replace":{"type":"rect","id":"variant001","name":"TEST DATA","width":8,"height":8}}}})).unwrap()];
    let Node::Instance(n) = doc.pages[0].children.last_mut().unwrap() else {
        panic!()
    };
    n.variant.insert("State".into(), "large".into());
    n.overrides.insert(
        path("croot00001/crect00001"),
        vellum_types::Override::Props(patch(json!({"width":12}))),
    );
    let p = planned(
        &doc,
        vec![
            Op::Scale {
                id: id("inst000001"),
                factor: 2.,
                anchor: Point::default(),
            },
            Op::Detach {
                id: id("inst000001"),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        find(&p.document, "inst000001").unwrap().children()[0].width(),
        &Size::Fixed(Value::Literal(24.))
    );
    inverse(&doc, &p);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(256))]
    #[test]
    fn mixed_component_and_geometry_batches_keep_invariants_and_exact_inverses(steps in proptest::collection::vec(0u8..8,1..20)) {
        let mut doc=document();let mut ids=Ids::default();
        for step in steps {
            let op=match step {
                0=>Op::CreateComponent{id:id("frame00001"),component_id:component_id("comp000002")},
                1=>Op::ToggleComponent{id:id("frame00001"),component_id:component_id("comp000002")},
                2=>Op::Detach{id:id("frame00001")},
                3=>Op::Scale{id:id("frame00001"),factor:2.,anchor:Point{x:0.5,y:0.5}},
                4=>Op::Scale{id:id("frame00001"),factor:0.5,anchor:Point::default()},
                5=>Op::Flip{id:id("frame00001"),axis:Axis::Horizontal},
                6=>Op::Flip{id:id("frame00001"),axis:Axis::Vertical},
                _=>Op::MakeSlot{id:id("rect000003"),suggested:vec![]},
            };
            let batch=tx(vec![Op::Flip{id:id("rect000003"),axis:Axis::Horizontal},op]);
            match ops::plan(&doc,&batch,&mut ids) {
                Ok(plan)=>{inverse(&doc,&plan);doc=plan.document;},
                Err(_)=>{let mut store=Store::new(doc.clone());assert!(ops::apply(&mut store,&mut ids,batch).is_err());assert_eq!(store.doc,doc);assert_eq!(store.applied,0);}
            }
        }
    }
}
