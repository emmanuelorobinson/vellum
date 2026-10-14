//! Random valid edits, driven by bytes: the same bytes give the same edits, so a proptest case or a fuzz input
//! replays exactly. Every edit goes through `ops::plan`, so every document made here is one the op layer
//! accepts. Shared by `tests/diff_merge.rs` and the fuzz target in `fuzz/` (`#[path]`-included there).
//!
//! Everything here is `TEST DATA`.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value as Json, json};
use vellum_core::ops::{self, Axis, Op, Parent, Place, Shell};
use vellum_core::ports::IdGen;
use vellum_types::{
    Actor, Component, ComponentId, DeviceId, Document, Node, NodeId, NodeKind, NodePatch, Origin,
    Page, PageId, PageKind, Variable, VariableId,
};

/// Bytes read as choices. Past the end every choice is 0.
pub struct Bytes<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Bytes<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    pub fn byte(&mut self) -> u8 {
        let b = self.data.get(self.at).copied().unwrap_or(0);
        self.at += 1;
        b
    }

    /// A number below `n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        let wide = (usize::from(self.byte()) << 8) | usize::from(self.byte());
        wide % n
    }

    pub fn left(&self) -> bool {
        self.at < self.data.len()
    }

    fn pick<'b, T>(&mut self, items: &'b [T]) -> Option<&'b T> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }
}

/// `len` bytes from SplitMix64 seeded with `seed`: an input for [`Bytes`] when there is no fuzzer or proptest.
pub fn seeded(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) as u8
        })
        .collect()
}

/// Fresh ids with a tag, so two branches made from one document never pick the same new id by chance.
pub struct TagIds {
    tag: String,
    next: u32,
}

impl TagIds {
    pub fn new(tag: &str) -> Self {
        Self {
            tag: tag.into(),
            next: 0,
        }
    }

    fn text(&mut self) -> String {
        self.next += 1;
        format!("{}{:05}", self.tag, self.next)
    }
}

impl IdGen for TagIds {
    fn node_id(&mut self) -> NodeId {
        NodeId::new(self.text()).expect("a valid id")
    }
}

pub fn origin() -> Origin {
    Origin::new(Actor::Person {
        device: DeviceId::new("01JA0000000000000000000A11").expect("a ULID"),
    })
}

/// The documents edits start from: every fixture in `fixtures/documents/` (`dir`), and a small one of every
/// kind of container.
pub fn bases(dir: &Path) -> Vec<Document> {
    let mut docs: Vec<Document> = std::fs::read_dir(dir)
        .expect("the fixture documents")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "vellum"))
        .map(|p| {
            let text = std::fs::read_to_string(&p).expect("readable");
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        })
        .collect();
    docs.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
    docs.push(small());
    docs
}

fn small() -> Document {
    serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA small", "createdWith": "tests"},
        "variables": {
            "color00001": {"type": "color", "name": "color/primary", "values": [{"value": "#3366FF"}]}
        },
        "components": {
            "comp000001": {"name": "Button", "root": {
                "type": "frame", "id": "croot00001", "name": "Button", "width": "hug", "height": "hug",
                "children": [{"type": "rect", "id": "crect00001", "name": "Bg", "width": 4, "height": 4}]
            }}
        },
        "pages": [
            {"id": "canvas0001", "name": "Canvas", "children": [
                {"type": "frame", "id": "frame00001", "name": "Card", "width": 300, "height": 200,
                 "children": [
                    {"type": "rect", "id": "rect000001", "name": "A", "width": 10, "height": 10},
                    {"type": "rect", "id": "rect000002", "name": "B", "width": 10, "height": 10}
                 ]},
                {"type": "group", "id": "group00001", "name": "G", "width": "hug", "height": "hug",
                 "children": [{"type": "ellipse", "id": "ellipse001", "name": "E", "width": 10, "height": 10}]},
                {"type": "instance", "id": "inst000001", "name": "I", "width": "hug", "height": "hug",
                 "componentId": "comp000001"},
                {"type": "text", "id": "text000001", "name": "T", "width": "hug", "height": "hug",
                 "content": [{"text": "Hi", "font": "Inter", "size": 14}],
                 "fills": [{"type": "solid", "color": {"$var": "color00001"}}]}
            ]},
            {"id": "slides0001", "name": "Slides", "kind": "slides", "children": [
                {"type": "frame", "id": "slide00001", "name": "Slide 1", "width": 1920, "height": 1080}
            ]}
        ]
    }))
    .expect("the small document is valid")
}

/// Every node in the pages and components, with what kind it is.
pub fn nodes(doc: &Document) -> Vec<(NodeId, NodeKind)> {
    fn walk(nodes: &[Node], out: &mut Vec<(NodeId, NodeKind)>) {
        for n in nodes {
            out.push((n.id().clone(), n.kind()));
            walk(n.children(), out);
        }
    }
    let mut out = Vec::new();
    for page in &doc.pages {
        walk(&page.children, &mut out);
    }
    for c in doc.components.values() {
        walk(std::slice::from_ref(&c.root), &mut out);
    }
    out
}

fn places(doc: &Document) -> Vec<Parent> {
    let mut out: Vec<Parent> = doc
        .pages
        .iter()
        .map(|p| Parent::Page(p.id.clone()))
        .collect();
    out.extend(
        nodes(doc)
            .into_iter()
            .filter(|(_, k)| matches!(k, NodeKind::Frame | NodeKind::Group))
            .map(|(id, _)| Parent::Node(id)),
    );
    out
}

fn patch(value: Json) -> NodePatch {
    let Json::Object(map) = value else {
        panic!("a patch is an object")
    };
    NodePatch::new(map.into_iter().collect::<BTreeMap<_, _>>()).expect("known properties")
}

fn leaf(kind: &str, id: &str, n: usize) -> Node {
    let mut value = json!({"type": kind, "id": id, "name": format!("TEST DATA {id}"),
                           "width": 10 + n % 7, "height": 10});
    match kind {
        "text" => value["content"] = json!([{"text": "TEST DATA", "font": "Inter", "size": 12}]),
        "polygon" => value["sides"] = json!(3 + n % 4),
        "star" => {
            value["points"] = json!(5);
            value["innerRatio"] = json!(0.5);
        }
        _ => {}
    }
    serde_json::from_value(value).expect("a valid leaf")
}

fn children_len(doc: &Document, parent: &Parent) -> usize {
    match parent {
        Parent::Page(p) => doc
            .pages
            .iter()
            .find(|x| &x.id == p)
            .map_or(0, |x| x.children.len()),
        Parent::Node(n) => ops::find(doc, n).map_or(0, |x| x.children().len()),
    }
}

/// One random op for `doc`, or None when the bytes pick something with nothing to act on.
pub fn one_op(doc: &Document, bytes: &mut Bytes<'_>, ids: &mut TagIds) -> Option<Op> {
    let all = nodes(doc);
    let target = bytes.pick(&all).map(|(id, _)| id.clone());
    let n = bytes.below(1000);
    Some(match bytes.below(20) {
        0..=3 => {
            let key = ["name", "opacity", "rotation", "visible", "fills", "locked"][bytes.below(6)];
            let value = match key {
                "name" => json!(format!("TEST DATA {}{n}", ids.tag)),
                "opacity" => json!((1 + n % 9) as f64 / 10.0),
                "rotation" => json!((n % 36) as f64 * 10.0),
                "visible" => json!(n.is_multiple_of(2)),
                "locked" => json!(n % 2 == 1),
                _ => {
                    let color = ["#FF0000", "#00FF00", "#0000FF"][n % 3];
                    json!([{"type": "solid", "color": color}])
                }
            };
            Op::Update {
                id: target?,
                patch: patch(json!({ key: value })),
            }
        }
        4..=6 => {
            let parent = bytes.pick(&places(doc))?.clone();
            let kind = ["rect", "frame", "ellipse", "text", "group", "star"][bytes.below(6)];
            let id = ids.text();
            let node = if kind == "group" {
                serde_json::from_value(json!({"type": "group", "id": id, "name": "TEST DATA g",
                    "width": "hug", "height": "hug", "children": [leaf("rect", &ids.text(), n)]}))
                .expect("a group")
            } else {
                leaf(kind, &id, n)
            };
            let index = bytes.below(children_len(doc, &parent) + 1);
            Op::Insert {
                to: Place { parent, index },
                node: Box::new(node),
            }
        }
        7 | 8 => Op::Delete { id: target? },
        9 | 10 => {
            let parent = bytes.pick(&places(doc))?.clone();
            let index = bytes.below(children_len(doc, &parent) + 1);
            Op::Move {
                id: target?,
                to: Place { parent, index },
            }
        }
        11 => Op::Reorder {
            id: target?,
            index: bytes.below(4),
        },
        12 => {
            let id = target?;
            let kind = ["rect", "ellipse", "frame", "text"][bytes.below(4)];
            Op::Replace {
                node: Box::new(leaf(kind, id.as_str(), n)),
                id,
            }
        }
        13 => {
            let vars: Vec<&VariableId> = doc.variables.keys().collect();
            match bytes.below(3) {
                0 => Op::SetVariables {
                    set: BTreeMap::from([(
                        VariableId::new(ids.text()).expect("an id"),
                        variable(json!({"type": "number", "name": format!("TEST DATA {n}"),
                                        "values": [{"value": n}]})),
                    )]),
                    remove: BTreeSet::new(),
                },
                1 => {
                    let id = (*bytes.pick(&vars)?).clone();
                    let mut value = serde_json::to_value(&doc.variables[&id]).expect("a variable");
                    value["name"] = json!(format!("TEST DATA {}{n}", ids.tag));
                    Op::SetVariables {
                        set: BTreeMap::from([(id, variable(value))]),
                        remove: BTreeSet::new(),
                    }
                }
                _ => Op::SetVariables {
                    set: BTreeMap::new(),
                    remove: BTreeSet::from([(*bytes.pick(&vars)?).clone()]),
                },
            }
        }
        14 => {
            let pages: Vec<&PageId> = doc.pages.iter().map(|p| &p.id).collect();
            match bytes.below(4) {
                0 => Op::InsertPage {
                    index: bytes.below(pages.len() + 1),
                    page: Box::new(Page {
                        id: PageId::new(ids.text()).expect("an id"),
                        name: format!("TEST DATA page {n}"),
                        kind: if n.is_multiple_of(3) {
                            PageKind::Slides
                        } else {
                            PageKind::Canvas
                        },
                        children: vec![leaf("frame", &ids.text(), n)],
                    }),
                },
                1 => Op::DeletePage {
                    id: (*bytes.pick(&pages)?).clone(),
                },
                2 => {
                    let page = bytes.pick(&doc.pages)?;
                    Op::UpdatePage {
                        id: page.id.clone(),
                        name: format!("TEST DATA {}{n}", ids.tag),
                        kind: if n.is_multiple_of(4) {
                            PageKind::Slides
                        } else {
                            page.kind
                        },
                    }
                }
                _ => Op::MovePage {
                    id: (*bytes.pick(&pages)?).clone(),
                    index: bytes.below(pages.len()),
                },
            }
        }
        15 => {
            let mut shell = Shell::of(doc);
            match bytes.below(3) {
                0 => shell.meta.name = format!("TEST DATA {}{n}", ids.tag),
                1 => {
                    shell
                        .themes
                        .insert(format!("axis{}", n % 3), vec!["a".into(), format!("v{n}")]);
                }
                _ => {
                    shell.imports.insert(
                        format!("lib{}", n % 3),
                        format!("libs/{}{n}.lib.vellum", ids.tag),
                    );
                }
            }
            Op::SetShell {
                shell: Box::new(shell),
            }
        }
        16 => {
            let comps: Vec<&ComponentId> = doc.components.keys().collect();
            match bytes.below(4) {
                0 => {
                    let root = ids.text();
                    let component: Component = serde_json::from_value(json!({
                        "name": format!("TEST DATA {n}"),
                        "root": {"type": "frame", "id": root, "name": "Root", "width": 10, "height": 10,
                                 "children": [leaf("rect", &ids.text(), n)]}
                    }))
                    .expect("a component");
                    Op::SetComponent {
                        id: ComponentId::new(ids.text()).expect("an id"),
                        component: Some(Box::new(component)),
                    }
                }
                1 => {
                    let id = (*bytes.pick(&comps)?).clone();
                    let c = &doc.components[&id];
                    Op::UpdateComponent {
                        id,
                        name: format!("TEST DATA {}{n}", ids.tag),
                        props: c.props.clone(),
                        variants: c.variants.clone(),
                    }
                }
                2 => {
                    let component = (*bytes.pick(&comps)?).clone();
                    let parent = bytes.pick(&places(doc))?.clone();
                    let node = serde_json::from_value(json!({"type": "instance", "id": ids.text(),
                        "name": "TEST DATA i", "width": "hug", "height": "hug",
                        "componentId": component}))
                    .expect("an instance");
                    Op::Insert {
                        to: Place { parent, index: 0 },
                        node: Box::new(node),
                    }
                }
                _ => Op::SetComponent {
                    id: (*bytes.pick(&comps)?).clone(),
                    component: None,
                },
            }
        }
        17 => Op::Group {
            ids: vec![target?],
            group: Box::new(
                serde_json::from_value(
                    json!({"type": "group", "id": ids.text(), "name": "TEST DATA g",
                    "width": "hug", "height": "hug"}),
                )
                .expect("a group"),
            ),
        },
        18 => {
            let groups: Vec<NodeId> = all
                .iter()
                .filter(|(_, k)| *k == NodeKind::Group)
                .map(|(id, _)| id.clone())
                .collect();
            Op::Ungroup {
                id: bytes.pick(&groups)?.clone(),
            }
        }
        _ => match bytes.below(3) {
            0 => Op::Copy {
                id: target?,
                to: Place {
                    parent: bytes.pick(&places(doc))?.clone(),
                    index: 0,
                },
            },
            1 => Op::Flip {
                id: target?,
                axis: Axis::Vertical,
            },
            _ => Op::CreateComponent {
                id: target?,
                component_id: ComponentId::new(ids.text()).expect("an id"),
            },
        },
    })
}

fn variable(value: Json) -> Variable {
    serde_json::from_value(value).expect("a valid variable")
}

/// `doc` after `steps` random transactions (of one to three ops each) that the op layer accepts. Refused ones
/// are skipped.
pub fn mutate(doc: &Document, bytes: &mut Bytes<'_>, tag: &str, steps: usize) -> Document {
    mutate_counting(doc, bytes, tag, steps, &mut BTreeMap::new())
}

/// As [`mutate`], counting the accepted ops by kind into `accepted`.
pub fn mutate_counting(
    doc: &Document,
    bytes: &mut Bytes<'_>,
    tag: &str,
    steps: usize,
    accepted: &mut BTreeMap<String, usize>,
) -> Document {
    let mut doc = doc.clone();
    let mut ids = TagIds::new(tag);
    for _ in 0..steps {
        let count = 1 + bytes.below(3);
        let ops: Vec<Op> = (0..count)
            .filter_map(|_| one_op(&doc, bytes, &mut ids))
            .collect();
        if ops.is_empty() {
            continue;
        }
        let names: Vec<String> = ops.iter().map(op_name).collect();
        let tx = ops::Transaction::new(origin(), ops);
        if let Ok(plan) = ops::plan(&doc, &tx, &mut ids) {
            doc = plan.document;
            for name in names {
                *accepted.entry(name).or_default() += 1;
            }
        }
    }
    doc
}

/// As [`mutate`], but each accepted transaction on its own: the document it leaves and what it changed. `extra`
/// may offer an op of its own before the shared generator is asked (it gets the document as it stands).
pub fn mutate_steps(
    doc: &Document,
    bytes: &mut Bytes<'_>,
    tag: &str,
    steps: usize,
    extra: &mut dyn FnMut(&Document, &mut Bytes<'_>, &mut TagIds) -> Option<Op>,
) -> Vec<(Document, vellum_core::ports::ChangeSet)> {
    let mut doc = doc.clone();
    let mut ids = TagIds::new(tag);
    let mut out = Vec::new();
    for _ in 0..steps {
        let count = 1 + bytes.below(3);
        let ops: Vec<Op> = (0..count)
            .filter_map(|_| {
                if bytes.below(3) == 0 {
                    extra(&doc, bytes, &mut ids).or_else(|| one_op(&doc, bytes, &mut ids))
                } else {
                    one_op(&doc, bytes, &mut ids)
                }
            })
            .collect();
        if ops.is_empty() {
            continue;
        }
        let tx = ops::Transaction::new(origin(), ops);
        if let Ok(plan) = ops::plan(&doc, &tx, &mut ids) {
            doc = plan.document.clone();
            out.push((plan.document, plan.changes));
        }
    }
    out
}

/// The op's variant name, as `Insert`.
pub fn op_name(op: &Op) -> String {
    let text = format!("{op:?}");
    text.split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_string()
}

/// A base document and two versions of it, both made from `data`.
pub fn case(bases: &[Document], data: &[u8]) -> (Document, Document, Document) {
    let mut bytes = Bytes::new(data);
    let base = bases[bytes.below(bases.len())].clone();
    let steps = 1 + bytes.below(6);
    let ours = mutate(&base, &mut bytes, "o", steps);
    let steps = 1 + bytes.below(6);
    let theirs = mutate(&base, &mut bytes, "t", steps);
    (base, ours, theirs)
}

/// `diff(old, new)`, planned on `old`, gives `new`.
pub fn check_diff(old: &Document, new: &Document) {
    let ops = vellum_core::diff::diff(old, new)
        .unwrap_or_else(|e| panic!("diff failed: {e} at {}", e.location()));
    if ops.is_empty() {
        assert_eq!(old, new, "no ops, but the documents differ");
        return;
    }
    let plan = ops::plan(
        old,
        &ops::Transaction::new(origin(), ops.clone()),
        &mut TagIds::new("d"),
    )
    .unwrap_or_else(|e| panic!("the diff was refused: {e} at {}\n{ops:#?}", e.location()));
    assert!(
        &plan.document == new,
        "the diff does not lead to the new document\n{ops:#?}"
    );
}

/// The merge properties for one case. Returns whether `merge(base, ours, theirs)` was clean.
pub fn check_merge(base: &Document, ours: &Document, theirs: &Document) -> bool {
    use vellum_core::merge::merge;
    for (x, label) in [(ours, "ours"), (theirs, "theirs")] {
        let one = merge(base, x, base).unwrap_or_else(|e| panic!("merge(b, {label}, b): {e}"));
        assert!(one.document == *x, "merge(b, {label}, b) is not {label}");
        assert!(
            one.conflicts.is_empty(),
            "merge(b, {label}, b) conflicted: {:?}",
            one.conflicts
        );
        let other = merge(base, base, x).unwrap_or_else(|e| panic!("merge(b, b, {label}): {e}"));
        assert!(other.document == *x, "merge(b, b, {label}) is not {label}");
        assert!(other.conflicts.is_empty());
    }
    let merged = merge(base, ours, theirs).unwrap_or_else(|e| panic!("merge(b, o, t): {e}"));
    let problems = ops::check_document(&merged.document);
    assert!(problems.is_empty(), "the merge is not valid: {problems:?}");
    // The editor applies a merge to the open document as a diff.
    check_diff(ours, &merged.document);
    let clean = merged.conflicts.is_empty();
    if clean {
        let swapped = merge(base, theirs, ours).unwrap_or_else(|e| panic!("merge(b, t, o): {e}"));
        assert!(
            swapped.conflicts.is_empty(),
            "clean one way, conflicted the other: {:?}",
            swapped.conflicts
        );
        assert!(
            swapped.document == merged.document,
            "a clean merge depends on which side is ours"
        );
    }
    clean
}
