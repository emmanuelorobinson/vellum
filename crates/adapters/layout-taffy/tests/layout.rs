//! TEST DATA: production LayoutEngine contract, incremental measurement and browser oracle.
use std::{cell::RefCell, collections::BTreeMap, path::Path};
use vellum_contracts::{fakes::FixedMetricText, suites};
use vellum_core::{
    layout::{Axis, LayoutProblem},
    ports::*,
};
use vellum_layout_taffy::TaffyLayout;
use vellum_types::{Layout, NodeIdPath};
fn id(s: &str) -> NodeIdPath {
    s.parse().unwrap()
}
fn node(
    s: &str,
    mode: Layout,
    w: Dimension,
    h: Dimension,
    children: Vec<LayoutNode>,
) -> LayoutNode {
    LayoutNode {
        id: id(s),
        style: LayoutStyle {
            mode,
            width: w,
            height: h,
            ..Default::default()
        },
        content: LayoutContent::Frame,
        children,
    }
}
const HUG: Dimension = Dimension::Hug { fallback: None };
const FILL: Dimension = Dimension::Fill { fallback: None };
fn text(s: &str, value: &str) -> LayoutNode {
    let mut n = node(s, Layout::None, HUG, HUG, vec![]);
    n.content = LayoutContent::Text(RichText::plain(value, "TEST DATA Sans", 10.0));
    n
}
#[test]
fn layout_engine_contract() {
    suites::layout::run(TaffyLayout::new, &FixedMetricText::new());
}
struct CountText {
    calls: RefCell<BTreeMap<String, usize>>,
}
impl TextEngine for CountText {
    fn measure(&self, text: &RichText, extent: Extent) -> TextMetrics {
        *self.calls.borrow_mut().entry(text.text()).or_default() += 1;
        FixedMetricText::new().measure(text, extent)
    }
    fn layout(&self, text: &RichText, width: f64) -> TextLayout {
        FixedMetricText::new().layout(text, width)
    }
}
#[test]
fn incremental_reuses_clean_subtrees_but_remeasures_dependent_siblings() {
    let text_engine = CountText {
        calls: RefCell::default(),
    };
    let mut tree = LayoutTree {
        root: node(
            "root",
            Layout::None,
            Dimension::Fixed(600.0),
            Dimension::Fixed(400.0),
            vec![
                node(
                    "a",
                    Layout::Column,
                    Dimension::Fixed(200.0),
                    HUG,
                    vec![text("a/t", "left")],
                ),
                node(
                    "b",
                    Layout::Column,
                    Dimension::Fixed(200.0),
                    HUG,
                    vec![text("b/t", "right")],
                ),
            ],
        ),
    };
    let mut engine = TaffyLayout::new();
    engine.layout(&tree, &DirtySet::all(), &text_engine);
    text_engine.calls.borrow_mut().clear();
    let unchanged = engine.layout(&tree, &DirtySet::default(), &text_engine);
    assert!(text_engine.calls.borrow().is_empty());
    tree.root.children[0].children[0].content =
        LayoutContent::Text(RichText::plain("changed", "TEST DATA Sans", 10.0));
    let updated = engine.layout(&tree, &DirtySet::of([id("a/t")]), &text_engine);
    assert!(!text_engine.calls.borrow().contains_key("right"));
    assert!(text_engine.calls.borrow().contains_key("changed"));
    assert_ne!(unchanged, updated);
    assert_eq!(
        updated,
        TaffyLayout::new().layout(&tree, &DirtySet::all(), &text_engine)
    );
    text_engine.calls.borrow_mut().clear();
    engine.layout(&tree, &DirtySet::all(), &text_engine);
    assert!(text_engine.calls.borrow().contains_key("right"));
}
#[test]
fn hug_dependency_is_detected_and_broken_by_adr002() {
    let tree = LayoutTree {
        root: node(
            "r",
            Layout::Row,
            HUG,
            HUG,
            vec![node(
                "c",
                Layout::Row,
                FILL,
                HUG,
                vec![node(
                    "d",
                    Layout::None,
                    Dimension::Fixed(40.0),
                    Dimension::Fixed(20.0),
                    vec![],
                )],
            )],
        ),
    };
    let r = TaffyLayout::new().layout(&tree, &DirtySet::all(), &FixedMetricText::new());
    assert_eq!(r.boxes[&id("r")].width, 40.0);
    assert!(
        matches!(&r.problems[..], [LayoutProblem::HugCycleBroken { parent, child, axis: Axis::Width }] if parent == &id("r") && child == &id("c"))
    );
}
#[test]
fn stroke_padding_absolute_anchor_and_visibility() {
    let mut root = node(
        "r",
        Layout::Row,
        Dimension::Fixed(120.0),
        Dimension::Fixed(80.0),
        vec![
            node("c", Layout::None, FILL, FILL, vec![]),
            node(
                "abs",
                Layout::None,
                Dimension::Fixed(10.0),
                Dimension::Fixed(10.0),
                vec![],
            ),
            node(
                "hidden",
                Layout::None,
                Dimension::Fixed(500.0),
                Dimension::Fixed(10.0),
                vec![],
            ),
        ],
    );
    root.style.layout_includes_stroke = true;
    root.style.stroke = Insets {
        top: 2.0,
        right: 4.0,
        bottom: 6.0,
        left: 8.0,
    };
    root.children[1].style.absolute = true;
    root.children[1].style.x = 3.0;
    root.children[1].style.y = -5.0;
    root.children[2].style.visible = false;
    let r = TaffyLayout::new().layout(
        &LayoutTree { root },
        &DirtySet::all(),
        &FixedMetricText::new(),
    );
    assert_eq!(r.boxes[&id("c")], Rect::new(8.0, 2.0, 108.0, 72.0));
    assert_eq!(r.boxes[&id("abs")], Rect::new(3.0, -5.0, 10.0, 10.0));
    assert!(!r.boxes.contains_key(&id("hidden")));
}
#[test]
fn oracle_at_least_150_fixtures_match_chrome() {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/layout-oracle/expected");
    let fixtures = vellum_testkit::layout_oracle::load_dir(&dir).unwrap();
    assert!(fixtures.len() >= 150, "only {} fixtures", fixtures.len());
    for fixture in fixtures {
        let tree = vellum_testkit::layout_oracle::layout_tree(&fixture).root;
        let result = TaffyLayout::new().layout(
            &LayoutTree { root: tree },
            &DirtySet::all(),
            &FixedMetricText::new(),
        );
        let actual = result
            .boxes
            .into_iter()
            .map(|(id, r)| {
                (
                    id.to_string().replace('/', "."),
                    vellum_testkit::layout_oracle::LayoutBox::new(r.x, r.y, r.width, r.height),
                )
            })
            .collect();
        let diff = vellum_testkit::layout_oracle::compare(&fixture, &actual, 0.5);
        assert!(diff.is_empty(), "{}: {diff:?}", fixture.fixture);
    }
}

#[test]
fn dependent_text_rewraps_when_sibling_takes_space() {
    let counter = CountText {
        calls: RefCell::default(),
    };
    let mut wrapping = text("t", "hello brave new world");
    wrapping.style.width = FILL;
    let mut tree = LayoutTree {
        root: node(
            "r",
            Layout::Row,
            Dimension::Fixed(160.0),
            HUG,
            vec![
                node(
                    "s",
                    Layout::None,
                    Dimension::Fixed(20.0),
                    Dimension::Fixed(10.0),
                    vec![],
                ),
                wrapping,
            ],
        ),
    };
    let mut engine = TaffyLayout::new();
    let first = engine.layout(&tree, &DirtySet::all(), &counter);
    counter.calls.borrow_mut().clear();
    tree.root.children[0].style.width = Dimension::Fixed(120.0);
    let result = engine.layout(&tree, &DirtySet::of([id("s")]), &counter);
    assert!(counter.calls.borrow().contains_key("hello brave new world"));
    assert!(result.boxes[&id("t")].height > first.boxes[&id("t")].height);
    assert_eq!(
        result,
        TaffyLayout::new().layout(&tree, &DirtySet::all(), &counter)
    );
}
#[test]
fn structural_edits_remove_old_boxes_and_keep_occurrences_distinct() {
    let mut tree = LayoutTree {
        root: node(
            "r",
            Layout::Column,
            Dimension::Fixed(200.0),
            HUG,
            vec![
                node("one", Layout::Row, FILL, HUG, vec![text("one/t", "one")]),
                node("two", Layout::Row, FILL, HUG, vec![text("two/t", "two")]),
            ],
        ),
    };
    let mut engine = TaffyLayout::new();
    let text_engine = FixedMetricText::new();
    let first = engine.layout(&tree, &DirtySet::all(), &text_engine);
    assert_eq!(first.boxes.len(), 5);
    let moved = tree.root.children[0].children.remove(0);
    tree.root.children[1].children.push(moved);
    tree.root.children[0].style.visible = false;
    tree.root.children[1].children[0].style.width = Dimension::Fixed(40.0);
    let result = engine.layout(&tree, &DirtySet::default(), &text_engine);
    assert_eq!(
        result,
        TaffyLayout::new().layout(&tree, &DirtySet::all(), &text_engine)
    );
    assert!(!result.boxes.contains_key(&id("one")));
    tree.root.children.remove(0);
    tree.root.children[0].children.remove(0);
    let result = engine.layout(&tree, &DirtySet::default(), &text_engine);
    assert!(!result.boxes.contains_key(&id("two/t")));
    assert_eq!(
        result,
        TaffyLayout::new().layout(&tree, &DirtySet::all(), &text_engine)
    );
}
#[test]
fn reversing_old_ancestry_never_creates_a_transient_cycle() {
    let fixed = Dimension::Fixed(80.0);
    let leaf = node("b", Layout::Row, fixed, fixed, vec![]);
    let mut tree = LayoutTree {
        root: node(
            "r",
            Layout::None,
            Dimension::Fixed(200.0),
            Dimension::Fixed(200.0),
            vec![node("a", Layout::Row, fixed, fixed, vec![leaf])],
        ),
    };
    let mut engine = TaffyLayout::new();
    let text_engine = FixedMetricText::new();
    engine.layout(&tree, &DirtySet::all(), &text_engine);
    let mut a = tree.root.children.remove(0);
    let mut b = a.children.remove(0);
    b.children.push(a);
    tree.root.children.push(b);
    let result = engine.layout(&tree, &DirtySet::default(), &text_engine);
    assert_eq!(
        result,
        TaffyLayout::new().layout(&tree, &DirtySet::all(), &text_engine)
    );
}
proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn incremental_equals_fresh_after_mixed_edits(edits in proptest::collection::vec((0u8..10,1u16..200),1..40)) {
        let fixed=Dimension::Fixed;
        let mut tree=LayoutTree{root:node("r",Layout::Row,fixed(300.0),fixed(200.0),vec![node("a",Layout::Column,FILL,HUG,vec![text("a/t","hello world")]),node("b",Layout::Row,fixed(40.0),fixed(20.0),vec![])])};
        let mut engine=TaffyLayout::new(); let text_engine=FixedMetricText::new();
        engine.layout(&tree,&DirtySet::all(),&text_engine);
        for (kind,value) in edits {
            let v=value as f64;
            match kind {
                0=>tree.root.style.width=fixed(v),
                1=>tree.root.children[1].style.width=fixed(v),
                2=>tree.root.style.gap=v/10.0,
                3=>tree.root.style.align=match value%3 {0=>vellum_types::Align::Start,1=>vellum_types::Align::Center,_=>vellum_types::Align::End},
                4=>tree.root.children[1].style.visible=value%2==0,
                5=>tree.root.children[1].style.absolute=value%2==0,
                6=>{tree.root.style.mode=Layout::Grid;tree.root.style.columns=Some(2);},
                7=>{tree.root.style.mode=Layout::Row;tree.root.style.columns=None;},
                8=>{tree.root.style.layout_includes_stroke=true;tree.root.style.stroke.left=v/10.0;},
                _=>tree.root.children[0].children[0].content=LayoutContent::Text(RichText::plain("TEST DATA".repeat(value as usize%8),"TEST DATA Sans",10.0)),
            }
            let result=engine.layout(&tree,&DirtySet::default(),&text_engine);
            proptest::prop_assert_eq!(result,TaffyLayout::new().layout(&tree,&DirtySet::all(),&text_engine));
        }
    }
}
