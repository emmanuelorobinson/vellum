//! Property tests (guide Prompt 9 FINAL, Invariant 5): any document the types can hold saves to bytes that load
//! back and save as the same bytes; the order of the keys and the whitespace of the input never reach the
//! output; the same edits in any order give the same bytes; and rounding is idempotent.

use std::collections::{BTreeMap, HashSet};

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use serde_json::{Value as Json, json};
use vellum_codec_json::{Quantum, decode, encode, quantize};
use vellum_types::*;

#[path = "../../../vellum-types/tests/common/strategies.rs"]
mod strategies;

use strategies::*;

/// The node strategies are large tuples of nested strategies; building one in a debug build needs more
/// than the 2 MiB a test thread gets.
fn on_a_big_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(test)
        .expect("spawn")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

fn runner(cases: u32) -> TestRunner {
    TestRunner::new(Config {
        cases,
        ..Config::default()
    })
}

// --- generators ---------------------------------------------------------------------------------

fn arb_variable() -> impl Strategy<Value = Variable> {
    let name = "[a-z]{1,6}(/[a-z]{1,6}){0,2}";
    prop_oneof![
        (name, arb_f64()).prop_map(|(name, value)| Variable::Number(VariableBody {
            name,
            values: vec![VariableValue::Literal(LiteralEntry {
                value,
                when: BTreeMap::new()
            })],
        })),
        (name, arb_color()).prop_map(|(name, value)| Variable::Color(VariableBody {
            name,
            values: vec![VariableValue::Literal(LiteralEntry {
                value,
                when: BTreeMap::new()
            })],
        })),
    ]
}

fn arb_node_id_path() -> impl Strategy<Value = NodeIdPath> {
    vec(arb_id_text(), 1..4).prop_map(|ids| ids.join("/").parse().expect("ids with no slash"))
}

fn arb_prop_def() -> impl Strategy<Value = PropDef> {
    prop_oneof![
        any::<bool>().prop_map(|default| PropDef::Boolean(BooleanProp { default })),
        arb_text().prop_map(|default| PropDef::Text(TextProp { default })),
        arb_f64().prop_map(|default| PropDef::Number(NumberProp { default })),
    ]
}

fn arb_component() -> impl Strategy<Value = Component> {
    let variant = (
        btree_map("[A-Za-z]{1,6}", "[a-z]{1,6}", 0..3),
        btree_map(arb_node_id_path(), arb_override(arb_node().boxed()), 0..3),
    )
        .prop_map(|(properties, overrides)| Variant {
            properties,
            overrides,
        });
    (
        arb_text(),
        btree_map("[a-z]{1,6}", arb_prop_def(), 0..3),
        arb_node(),
        vec(variant, 0..2),
    )
        .prop_map(|(name, props, root, variants)| Component {
            name,
            props,
            root,
            variants,
        })
}

fn arb_page() -> impl Strategy<Value = Page> {
    (
        arb_id_text(),
        arb_text(),
        arb_enum(PageKind::ALL),
        vec(arb_node(), 0..3),
    )
        .prop_map(|(id, name, kind, children)| Page {
            id: PageId::new(id).expect("valid id"),
            name,
            kind,
            children,
        })
}

fn arb_document() -> impl Strategy<Value = Document> {
    (
        arb_text(),
        arb_enum(DocumentKind::ALL),
        btree_map("[a-z]{1,6}", "[a-z/]{1,12}\\.lib\\.vellum", 0..3),
        btree_map("[A-Za-z]{1,6}", vec("[a-z]{1,6}", 1..3), 0..3),
        btree_map(arb_id_text(), arb_variable(), 0..3),
        btree_map(arb_id_text(), arb_component(), 0..2),
        vec(arb_page(), 0..3),
    )
        .prop_map(
            |(name, kind, imports, themes, variables, components, pages)| {
                let mut doc = Document::new(name);
                doc.meta.kind = kind;
                doc.imports = imports;
                doc.themes = themes;
                doc.variables = variables
                    .into_iter()
                    .map(|(id, v)| (VariableId::new(id).expect("valid id"), v))
                    .collect();
                doc.components = components
                    .into_iter()
                    .map(|(id, c)| (ComponentId::new(id).expect("valid id"), c))
                    .collect();
                doc.pages = pages;
                doc
            },
        )
}

fn bytes(doc: &Document) -> Vec<u8> {
    encode(doc).expect("every generated number is finite")
}

fn as_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// --- load, save -----------------------------------------------------------------------------------

#[test]
fn saving_what_was_loaded_gives_the_same_bytes() {
    on_a_big_stack(|| {
        runner(48)
            .run(&arb_document(), |doc| {
                let first = bytes(&doc);
                let loaded = decode(&first).expect("what was written reads");
                let second = bytes(&loaded);
                prop_assert_eq!(as_text(&second), as_text(&first));
                Ok(())
            })
            .unwrap();
    });
}

#[test]
fn a_loaded_document_is_a_fixed_point_of_saving_and_loading() {
    on_a_big_stack(|| {
        runner(32)
            .run(&arb_document(), |doc| {
                let once = decode(&bytes(&doc)).expect("reads");
                let twice = decode(&bytes(&once)).expect("reads");
                prop_assert_eq!(&twice, &once);
                Ok(())
            })
            .unwrap();
    });
}

// --- order and whitespace never reach the bytes ---------------------------------------------------

/// A small deterministic generator, so a failing case replays.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// The same JSON with every object's keys in a random order and random whitespace, sometimes CRLF.
fn scramble(value: &Json, rng: &mut Lcg, out: &mut String) {
    fn space(rng: &mut Lcg, out: &mut String) {
        match rng.below(6) {
            0 => out.push(' '),
            1 => out.push_str("\r\n"),
            2 => out.push_str("\n\t"),
            _ => {}
        }
    }
    match value {
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                space(rng, out);
                scramble(item, rng, out);
            }
            space(rng, out);
            out.push(']');
        }
        Json::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            for i in (1..entries.len()).rev() {
                entries.swap(i, rng.below(i + 1));
            }
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                space(rng, out);
                out.push_str(&serde_json::to_string(key).expect("string"));
                space(rng, out);
                out.push(':');
                space(rng, out);
                scramble(item, rng, out);
            }
            space(rng, out);
            out.push('}');
        }
        other => out.push_str(&other.to_string()),
    }
}

#[test]
fn the_order_of_the_keys_and_the_whitespace_of_the_input_never_reach_the_output() {
    on_a_big_stack(|| {
        runner(48)
            .run(&(arb_document(), any::<u64>()), |(doc, seed)| {
                let canonical = bytes(&doc);
                let tree: Json = serde_json::from_slice(&canonical).expect("json");
                let mut text = String::new();
                scramble(&tree, &mut Lcg(seed), &mut text);
                let loaded = decode(text.as_bytes()).expect("the scrambled file reads");
                prop_assert_eq!(as_text(&bytes(&loaded)), as_text(&canonical));
                Ok(())
            })
            .unwrap();
    });
}

// --- the same edits in any order -------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Edit {
    Import(String, String),
    Theme(String, Vec<String>),
    Variable(VariableId, Variable),
    Component(ComponentId, Box<Component>),
    Metadata(String, Json),
}

impl Edit {
    /// What this edit sets; two edits of the same thing would not commute.
    fn target(&self) -> String {
        match self {
            Self::Import(key, _) => format!("import/{key}"),
            Self::Theme(key, _) => format!("theme/{key}"),
            Self::Variable(id, _) => format!("variable/{id}"),
            Self::Component(id, _) => format!("component/{id}"),
            Self::Metadata(key, _) => format!("metadata/{key}"),
        }
    }

    fn apply(self, doc: &mut Document) {
        match self {
            Self::Import(key, path) => {
                doc.imports.insert(key, path);
            }
            Self::Theme(key, values) => {
                doc.themes.insert(key, values);
            }
            Self::Variable(id, variable) => {
                doc.variables.insert(id, variable);
            }
            Self::Component(id, component) => {
                doc.components.insert(id, *component);
            }
            Self::Metadata(key, value) => {
                if let Some(Node::Rect(rect)) = doc.pages[0].children.first_mut() {
                    rect.metadata.insert(key, value);
                }
            }
        }
    }
}

fn arb_edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        ("[a-z]{1,6}", "[a-z]{1,8}\\.lib\\.vellum").prop_map(|(k, p)| Edit::Import(k, p)),
        ("[A-Za-z]{1,6}", vec("[a-z]{1,6}", 1..3)).prop_map(|(k, v)| Edit::Theme(k, v)),
        (arb_id_text(), arb_variable())
            .prop_map(|(id, v)| Edit::Variable(VariableId::new(id).expect("valid"), v)),
        (arb_id_text(), arb_component())
            .prop_map(|(id, c)| Edit::Component(ComponentId::new(id).expect("valid"), Box::new(c))),
        (arb_text(), arb_json()).prop_map(|(k, v)| Edit::Metadata(k, v)),
    ]
}

fn start() -> Document {
    let mut doc = Document::new("TEST DATA");
    doc.pages.push(Page {
        id: PageId::new("page000001").expect("valid"),
        name: "Canvas".to_owned(),
        kind: PageKind::Canvas,
        children: vec![
            serde_json::from_value(json!({
                "type": "rect", "id": "r0000001", "name": "Back", "width": 10.0, "height": 10.0
            }))
            .expect("a rect"),
        ],
    });
    doc
}

#[test]
fn the_same_edits_in_any_order_give_the_same_bytes() {
    on_a_big_stack(|| {
        let strategy = vec(arb_edit(), 1..8).prop_flat_map(|edits| {
            // Two edits of one thing do not commute, so each target is edited once.
            let mut seen = HashSet::new();
            let edits: Vec<Edit> = edits
                .into_iter()
                .filter(|edit| seen.insert(edit.target()))
                .collect();
            (Just(edits.clone()), Just(edits).prop_shuffle())
        });
        runner(48)
            .run(&strategy, |(in_order, shuffled)| {
                let (mut a, mut b) = (start(), start());
                for edit in in_order {
                    edit.apply(&mut a);
                }
                for edit in shuffled {
                    edit.apply(&mut b);
                }
                prop_assert_eq!(as_text(&bytes(&a)), as_text(&bytes(&b)));
                Ok(())
            })
            .unwrap();
    });
}

// --- what is not rounded survives exactly ----------------------------------------------------------

proptest! {
    #[test]
    fn free_form_metadata_survives_exactly(metadata in btree_map(arb_text(), arb_json(), 0..4)) {
        let mut doc = start();
        if let Some(Node::Rect(rect)) = doc.pages[0].children.first_mut() {
            rect.metadata = metadata.clone();
        }
        let back = decode(&bytes(&doc)).expect("reads");
        let Some(Node::Rect(rect)) = back.pages[0].children.first() else {
            panic!("the rect is still there");
        };
        prop_assert_eq!(&rect.metadata, &metadata);
    }

    #[test]
    fn any_text_survives_in_a_name(name in any::<String>()) {
        let mut doc = Document::new(name.clone());
        doc.pages.clear();
        let back = decode(&bytes(&doc)).expect("reads");
        prop_assert_eq!(back.meta.name, name);
    }

    #[test]
    fn rounding_is_idempotent_at_every_magnitude(
        value in prop_oneof![
            -1.0e6..1.0e6_f64,
            -1.0e15..1.0e15_f64,
            prop::num::f64::NORMAL | prop::num::f64::SUBNORMAL | prop::num::f64::ZERO,
        ],
        quantum in prop::sample::select(vec![Quantum::Pixels, Quantum::Ratio, Quantum::Degrees]),
    ) {
        let once = quantize(value, quantum);
        prop_assert!(once.is_finite());
        prop_assert_eq!(quantize(once, quantum), once);
    }

    #[test]
    fn rounding_moves_a_value_by_at_most_half_a_step(
        value in -1.0e6..1.0e6_f64,
        quantum in prop::sample::select(vec![Quantum::Pixels, Quantum::Ratio, Quantum::Degrees]),
    ) {
        let step = match quantum { Quantum::Ratio => 0.001, _ => 0.01 };
        prop_assert!((quantize(value, quantum) - value).abs() <= step / 2.0 + 1e-9);
    }
}
