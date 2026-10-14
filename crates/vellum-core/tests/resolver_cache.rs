//! The resolver's cache against a fresh resolution (guide Prompt 16): whatever random, accepted edits a
//! document goes through, telling the cache what changed and resolving again gives exactly what resolving
//! from nothing gives. `common/mutate.rs` makes the shared edits from bytes; the edits here are the ones that
//! matter to variables and themes: bindings, variable values, axes, and the theme a frame sets.
//!
//! Everything here is `TEST DATA`.

#![recursion_limit = "512"]

#[path = "common/mutate.rs"]
mod mutate;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use proptest::prelude::*;
use serde_json::{Value as Json, json};
use vellum_core::ops::{Op, Parent, Place, Shell};
use vellum_core::ports::IdGen;
use vellum_core::resolver::variables::{ResolvedNode, Resolver, Theme};
use vellum_types::{Document, NodeKind, NodePatch, Variable, VariableId, VariableType};

use mutate::{Bytes, TagIds, mutate_steps, nodes};

/// A document built to be resolved: two axes, a variable of each type (some aliases), frames that set themes,
/// and nodes bound to the variables.
fn themed() -> Document {
    serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA themed", "createdWith": "tests"},
        "themes": {"Mode": ["Light", "Dark"], "Device": ["Phone", "Tablet"]},
        "variables": {
            "col0000001": {"type": "color", "name": "color/surface", "values": [
                {"value": "#FFFFFF"}, {"value": "#111111", "when": {"Mode": "Dark"}}]},
            "col0000002": {"type": "color", "name": "color/alias", "values": [{"$alias": "col0000001"}]},
            "num0000001": {"type": "number", "name": "space/md", "values": [
                {"value": 8}, {"value": 12, "when": {"Device": "Tablet"}}]},
            "num0000002": {"type": "number", "name": "space/alias", "values": [
                {"value": 1}, {"$alias": "num0000001", "when": {"Mode": "Dark"}}]},
            "str0000001": {"type": "string", "name": "text/icon", "values": [{"value": "star"}]},
            "bol0000001": {"type": "boolean", "name": "flag/show", "values": [
                {"value": true}, {"value": false, "when": {"Mode": "Dark", "Device": "Tablet"}}]}
        },
        "pages": [{"id": "canvas0001", "name": "Canvas", "children": [
            {"type": "frame", "id": "root000001", "name": "Root", "width": 400, "height": 300,
             "fills": [{"type": "solid", "color": {"$var": "col0000002"}}],
             "children": [
                {"type": "frame", "id": "dark000001", "name": "Dark", "width": 100, "height": 100,
                 "theme": {"Mode": "Dark"}, "gap": {"$var": "num0000002"},
                 "children": [
                    {"type": "rect", "id": "rect000001", "name": "A", "width": 10, "height": 10,
                     "opacity": {"$var": "num0000001"}, "visible": {"$var": "bol0000001"},
                     "fills": [{"type": "solid", "color": {"$var": "col0000001"}}]},
                    {"type": "frame", "id": "tabl000001", "name": "Tablet", "width": 50, "height": 50,
                     "theme": {"Device": "Tablet"},
                     "children": [{"type": "rect", "id": "rect000002", "name": "B", "width": 5, "height": 5,
                                   "visible": {"$var": "bol0000001"}}]}
                 ]},
                {"type": "rect", "id": "rect000003", "name": "C", "width": 10, "height": 10,
                 "rotation": {"$var": "num0000001"}},
                {"type": "icon", "id": "icon000001", "name": "I", "width": 24, "height": 24, "library": "lucide",
                 "icon": {"$var": "str0000001"}, "weight": 400}
             ]}
        ]}]
    }))
    .expect("the themed document is valid")
}

fn bases() -> &'static [Document] {
    static BASES: OnceLock<Vec<Document>> = OnceLock::new();
    BASES.get_or_init(|| {
        let mut docs =
            mutate::bases(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/documents"));
        // The themed document twice, so most cases start from one that has something to resolve.
        docs.push(themed());
        docs.push(themed());
        docs
    })
}

fn patch(value: Json) -> NodePatch {
    let Json::Object(map) = value else {
        panic!("a patch is an object")
    };
    NodePatch::new(map.into_iter().collect::<BTreeMap<_, _>>()).expect("known properties")
}

fn variable(value: Json) -> Variable {
    serde_json::from_value(value).expect("a variable")
}

fn of_type(doc: &Document, kind: VariableType) -> Vec<VariableId> {
    doc.variables
        .iter()
        .filter(|(_, v)| v.variable_type() == kind)
        .map(|(id, _)| id.clone())
        .collect()
}

fn pick<'a, T>(bytes: &mut Bytes<'_>, items: &'a [T]) -> Option<&'a T> {
    (!items.is_empty()).then(|| &items[bytes.below(items.len())])
}

/// A condition on the document's axes, sometimes on a value it does not have.
fn when(doc: &Document, bytes: &mut Bytes<'_>) -> Json {
    let mut when = serde_json::Map::new();
    let axes: Vec<(&String, &Vec<String>)> = doc.themes.iter().collect();
    for _ in 0..bytes.below(3) {
        let Some((axis, values)) = pick(bytes, &axes) else {
            break;
        };
        let value = if bytes.below(8) == 0 {
            "Nowhere".to_owned()
        } else {
            pick(bytes, values).cloned().unwrap_or_default()
        };
        when.insert((*axis).clone(), json!(value));
    }
    Json::Object(when)
}

/// An edit that matters to the resolver.
fn resolver_op(doc: &Document, bytes: &mut Bytes<'_>, ids: &mut TagIds) -> Option<Op> {
    let all = nodes(doc);
    let (target, _) = pick(bytes, &all)?.clone();
    Some(match bytes.below(9) {
        // A frame sets (or stops setting) a theme value.
        0 | 1 => {
            let frames: Vec<_> = all.iter().filter(|(_, k)| *k == NodeKind::Frame).collect();
            let (id, _) = (*pick(bytes, &frames)?).clone();
            let axes: Vec<(&String, &Vec<String>)> = doc.themes.iter().collect();
            let theme = match pick(bytes, &axes) {
                Some((axis, values)) if bytes.below(5) > 0 => {
                    let value = if bytes.below(10) == 0 {
                        "Nowhere".into()
                    } else {
                        pick(bytes, values)?.clone()
                    };
                    json!({ (*axis).clone(): value })
                }
                _ => Json::Null,
            };
            Op::Update {
                id,
                patch: patch(json!({ "theme": theme })),
            }
        }
        // A property takes a variable, or goes back to a literal.
        2 | 3 => {
            let value = match bytes.below(4) {
                0 => json!({"opacity": pick(bytes, &of_type(doc, VariableType::Number))
                    .map_or(json!(1), |v| json!({"$var": v}))}),
                1 => json!({"visible": pick(bytes, &of_type(doc, VariableType::Boolean))
                    .map_or(json!(true), |v| json!({"$var": v}))}),
                2 => json!({"fills": pick(bytes, &of_type(doc, VariableType::Color))
                    .map_or(json!([]), |v| json!([{"type": "solid", "color": {"$var": v}}]))}),
                _ => json!({"opacity": 1, "visible": true}),
            };
            Op::Update {
                id: target,
                patch: patch(value),
            }
        }
        // A variable gets new entries of its own type: some under a condition, some aliases.
        4..=6 => {
            let keys: Vec<VariableId> = doc.variables.keys().cloned().collect();
            let id = pick(bytes, &keys)?.clone();
            let kind = doc.variables[&id].variable_type();
            let same = of_type(doc, kind);
            let mut values = Vec::new();
            for n in 0..1 + bytes.below(3) {
                let cond = when(doc, bytes);
                let mut entry = if bytes.below(4) == 0 {
                    json!({"$alias": pick(bytes, &same)?})
                } else {
                    let v = bytes.below(1000);
                    match kind {
                        VariableType::Color => {
                            json!({"value": format!("#{:06X}", v * 997 % 0x0100_0000)})
                        }
                        VariableType::Number => json!({"value": v}),
                        VariableType::String => json!({"value": format!("s{v}")}),
                        VariableType::Boolean => json!({"value": v.is_multiple_of(2)}),
                    }
                };
                if n > 0 || cond.as_object().is_some_and(|c| !c.is_empty()) {
                    entry["when"] = cond;
                }
                values.push(entry);
            }
            let name = doc.variables[&id].name().to_owned();
            Op::SetVariables {
                set: BTreeMap::from([(
                    id,
                    variable(json!({"type": kind.as_str(), "name": name, "values": values})),
                )]),
                remove: Default::default(),
            }
        }
        // The axes change: a value added, the order reversed (so the default moves), an axis dropped.
        7 => {
            let mut shell = Shell::of(doc);
            match bytes.below(3) {
                0 => {
                    let axis = ["Mode", "Device", "Brand"][bytes.below(3)].to_owned();
                    let value = format!("V{}", ids.node_id());
                    shell.themes.entry(axis).or_default().push(value);
                }
                1 => shell.themes.values_mut().for_each(|v| v.reverse()),
                _ => {
                    let key = shell.themes.keys().next().cloned();
                    if let Some(key) = key {
                        shell.themes.remove(&key);
                    }
                }
            }
            Op::SetShell {
                shell: Box::new(shell),
            }
        }
        // A new node bound to variables.
        _ => {
            let parents: Vec<_> = all
                .iter()
                .filter(|(_, k)| matches!(k, NodeKind::Frame | NodeKind::Group))
                .collect();
            let (parent, _) = (*pick(bytes, &parents)?).clone();
            let mut node = json!({"type": "rect", "id": ids.node_id(), "name": "TEST DATA bound",
                                  "width": 8, "height": 8});
            if let Some(v) = pick(bytes, &of_type(doc, VariableType::Number)) {
                node["opacity"] = json!({"$var": v});
            }
            if let Some(v) = pick(bytes, &of_type(doc, VariableType::Boolean)) {
                node["visible"] = json!({"$var": v});
            }
            Op::Insert {
                to: Place {
                    parent: Parent::Node(parent),
                    index: 0,
                },
                node: Box::new(serde_json::from_value(node).expect("a rect")),
            }
        }
    })
}

fn resolve_all(resolver: &mut Resolver, doc: &Document) -> Vec<Vec<Arc<ResolvedNode>>> {
    let theme = Theme::defaults(doc);
    let mut out: Vec<_> = doc
        .pages
        .iter()
        .map(|page| resolver.resolve_page(doc, page))
        .collect();
    out.push(
        doc.components
            .values()
            .map(|component| resolver.resolve_node(doc, &component.root, &theme))
            .collect(),
    );
    out
}

fn count(node: &Arc<ResolvedNode>) -> usize {
    1 + node.children.iter().map(count).sum::<usize>()
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 24..600)
}

/// Edits one document through `data`, resolving as it goes, and checks the cache against a fresh resolution
/// every time. Returns how many transactions were accepted.
fn run(data: &[u8]) -> usize {
    let mut input = Bytes::new(data);
    let base = bases()[input.below(bases().len())].clone();
    let mut resolver = Resolver::new();
    assert_eq!(
        resolve_all(&mut resolver, &base),
        resolve_all(&mut Resolver::new(), &base)
    );
    let count_steps = 2 + input.below(10);
    let steps = mutate_steps(&base, &mut input, "p", count_steps, &mut resolver_op);
    let mut waiting = 0;
    for (i, (doc, changes)) in steps.iter().enumerate() {
        resolver.invalidate(doc, changes);
        waiting += 1;
        // Sometimes several change sets pile up before the next resolve.
        if waiting < 3 && i % 3 == 0 && i + 1 < steps.len() {
            continue;
        }
        waiting = 0;
        let cached = resolve_all(&mut resolver, doc);
        let fresh = resolve_all(&mut Resolver::new(), doc);
        assert_eq!(cached, fresh, "after step {i}, with changes {changes:?}");
        let live: usize = cached.iter().flatten().map(count).sum();
        assert!(
            resolver.cached() <= live + 64 * (i + 1),
            "the cache holds {} entries for {live} live nodes",
            resolver.cached()
        );
    }
    steps.len()
}

/// Named `resolver::variables::…` so the guide's `cargo test resolver::variables` runs these with the unit tests.
mod resolver {
    mod variables {
        use super::super::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(300))]

            /// Resolving through the cache equals resolving from nothing, after any run of accepted edits.
            #[test]
            fn cached_equals_fresh(data in bytes()) {
                run(&data);
            }
        }

        /// A fixed corpus, so `cargo test` replays the same cases whatever proptest draws, and checks the generator
        /// reaches the changes the cache has to handle.
        #[test]
        fn a_fixed_corpus_agrees_and_gets_somewhere() {
            let accepted: usize = (0..400).map(|seed| run(&mutate::seeded(seed, 300))).sum();
            assert!(
                accepted > 800,
                "only {accepted} transactions were accepted; the generator is not reaching the resolver"
            );
        }
    }
}
