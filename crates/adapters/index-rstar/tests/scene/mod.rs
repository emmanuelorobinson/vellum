use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use vellum_core::{
    ports::{Rect, SpatialIndex},
    scene::*,
};
use vellum_index_rstar::RstarIndex;
use vellum_types::{NodeIdPath, Point};
fn id(s: &str) -> NodeIdPath {
    s.parse().unwrap()
}
fn node(s: &str, parent: Option<&str>, r: Rect) -> SceneNode {
    SceneNode {
        address: id(s),
        parent: parent.map(id),
        bounds: r,
        ..SceneNode::default_for(id(s))
    }
}
fn make(nodes: &[SceneNode]) -> Scene<RstarIndex> {
    let mut scene = Scene::new(RstarIndex::new());
    scene.sync(nodes).unwrap();
    scene
}
fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}
#[test]
fn spatial_contract() {
    vellum_contracts::suites::spatial::run(RstarIndex::new);
}
#[test]
fn point_rect_depth_and_drawing_order() {
    let nodes = [
        node("a", None, Rect::new(10., 20., 100., 100.)),
        node("a/b", Some("a"), Rect::new(5., 5., 20., 20.)),
        node("a/b/c", Some("a/b"), Rect::new(2., 2., 5., 5.)),
        node("z", None, Rect::new(15., 25., 4., 4.)),
    ];
    let s = make(&nodes);
    assert_eq!(s.hit_point(p(18., 28.), HitDepth::Deepest), Some(id("z")));
    assert_eq!(
        s.hit_point(p(20., 30.), HitDepth::Deepest),
        Some(id("a/b/c"))
    );
    assert_eq!(s.hit_point(p(20., 30.), HitDepth::TopLevel), Some(id("a")));
    assert_eq!(
        s.hit_rect(
            Rect::new(16., 26., 10., 10.),
            HitDepth::Deepest,
            Marquee::Contained
        ),
        vec![id("a/b/c")]
    );
    assert_eq!(
        s.hit_rect(
            Rect::new(20., 30., 1., 1.),
            HitDepth::TopLevel,
            Marquee::Intersects
        ),
        vec![id("a")]
    );
    assert_eq!(s.hit_point(p(-1., 0.), HitDepth::Deepest), None);
}
#[test]
fn clipping_visibility_lock_and_culling() {
    let mut root = node("r", None, Rect::new(0., 0., 20., 20.));
    root.clip = true;
    let child = node("r/c", Some("r"), Rect::new(15., 15., 20., 20.));
    let mut nodes = vec![root, child];
    let mut s = make(&nodes);
    assert_eq!(s.hit_point(p(25., 25.), HitDepth::Deepest), None);
    assert_eq!(s.hit_point(p(18., 18.), HitDepth::Deepest), Some(id("r/c")));
    assert!(
        s.hit_rect(
            Rect::new(23., 23., 2., 2.),
            HitDepth::Deepest,
            Marquee::Intersects
        )
        .is_empty()
    );
    nodes[0].locked = true;
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(18., 18.), HitDepth::Deepest), None);
    assert_eq!(
        s.cull(Rect::new(17., 17., 1., 1.)),
        vec![id("r"), id("r/c")]
    );
    nodes[0].visible = false;
    s.sync(&nodes).unwrap();
    assert!(s.cull(Rect::new(-100., -100., 200., 200.)).is_empty());
    nodes[0].visible = true;
    nodes[0].locked = false;
    nodes[0].clip = false;
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(25., 25.), HitDepth::TopLevel), Some(id("r")));
}
#[test]
fn rotated_boxes_clips_and_flipped_ancestors() {
    let mut root = node("r", None, Rect::new(100., 100., 100., 20.));
    root.rotation = 45.;
    root.clip = true;
    let child = node("r/c", Some("r"), Rect::new(0., 0., 100., 20.));
    let mut nodes = vec![root, child];
    let mut s = make(&nodes);
    assert_eq!(
        s.hit_point(p(150., 110.), HitDepth::Deepest),
        Some(id("r/c"))
    );
    assert_eq!(s.hit_point(p(110., 75.), HitDepth::Deepest), None);
    assert!(
        s.hit_rect(
            Rect::new(108., 68., 4., 4.),
            HitDepth::Deepest,
            Marquee::Intersects
        )
        .is_empty()
    );
    nodes[0].rotation = 0.;
    nodes[0].flip_x = true;
    nodes[1].bounds = Rect::new(0., 0., 10., 10.);
    s.sync(&nodes).unwrap();
    assert_eq!(
        s.hit_point(p(195., 105.), HitDepth::Deepest),
        Some(id("r/c"))
    );
    assert_eq!(s.hit_point(p(105., 105.), HitDepth::Deepest), Some(id("r")));
}
#[test]
fn incremental_moves_removal_reorder_and_invalid_updates_are_atomic() {
    let mut nodes = vec![
        node("a", None, Rect::new(0., 0., 10., 10.)),
        node("b", None, Rect::new(0., 0., 10., 10.)),
    ];
    let mut s = make(&nodes);
    assert_eq!(s.hit_point(p(2., 2.), HitDepth::Deepest), Some(id("b")));
    nodes.reverse();
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(2., 2.), HitDepth::Deepest), Some(id("a")));
    nodes[1].bounds.x = 50.;
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(2., 2.), HitDepth::Deepest), Some(id("b")));
    let good = nodes.clone();
    nodes[0].parent = Some(id("missing"));
    assert!(s.sync(&nodes).is_err());
    assert_eq!(s.hit_point(p(52., 2.), HitDepth::Deepest), Some(id("a")));
    nodes = good;
    nodes.truncate(1);
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(52., 2.), HitDepth::Deepest), None);
    nodes.push(nodes[0].clone());
    assert!(s.sync(&nodes).is_err());
    nodes.pop();
    nodes[0].bounds.x = f64::NAN;
    assert!(s.sync(&nodes).is_err());
    nodes[0].bounds.x = 0.;
    nodes[0].parent = Some(id("b"));
    assert!(s.sync(&nodes).is_err());
}
fn opts() -> SnapOptions {
    SnapOptions {
        search: Rect::new(-500., -500., 1000., 1000.),
        tolerance: 4.,
        zoom: 1.,
        edges: true,
        centres: false,
        equal_spacing: false,
        grid: None,
    }
}
#[test]
fn edges_centres_grid_zoom_and_exclusion() {
    let s = make(&[node("a", None, Rect::new(10., 30., 20., 20.))]);
    let mut o = opts();
    let r = s
        .snap(Rect::new(33., 32., 8., 8.), &BTreeSet::new(), o)
        .unwrap();
    assert_eq!(r.offset.x, -3.);
    assert_eq!(r.offset.y, -2.);
    assert!(r.guides.iter().all(|g| g.kind == SnapKind::Edge));
    o.zoom = 2.;
    assert_eq!(
        s.snap(Rect::new(33., 32., 8., 8.), &BTreeSet::new(), o)
            .unwrap()
            .offset
            .x,
        0.
    );
    o.zoom = 1.;
    o.edges = false;
    o.centres = true;
    assert_eq!(
        s.snap(Rect::new(17., 37., 8., 8.), &BTreeSet::new(), o)
            .unwrap()
            .offset,
        p(-1., -1.)
    );
    o.centres = false;
    o.grid = Some(1.);
    assert_eq!(
        s.snap(Rect::new(1.3, -2.7, 8., 8.), &BTreeSet::new(), o)
            .unwrap()
            .offset,
        p(-0.30000000000000004, -0.2999999999999998)
    );
    o = opts();
    assert_eq!(
        s.snap(Rect::new(33., 32., 8., 8.), &BTreeSet::from([id("a")]), o)
            .unwrap()
            .offset,
        p(0., 0.)
    );
    o.zoom = 0.;
    assert!(
        s.snap(Rect::new(0., 0., 1., 1.), &BTreeSet::new(), o)
            .is_err()
    );
}
#[test]
fn equal_spacing_between_and_beyond_neighbors_on_both_axes() {
    for vertical in [false, true] {
        let rect = |v: f64| {
            if vertical {
                Rect::new(0., v, 10., 10.)
            } else {
                Rect::new(v, 0., 10., 10.)
            }
        };
        let mut o = opts();
        o.edges = false;
        o.equal_spacing = true;
        let s = make(&[node("a", None, rect(0.)), node("b", None, rect(40.))]);
        let r = s.snap(rect(21.), &BTreeSet::new(), o).unwrap();
        assert_eq!(if vertical { r.offset.y } else { r.offset.x }, -1.);
        assert!(r.guides.iter().any(|g| g.kind == SnapKind::EqualSpacing));
        let r = s.snap(rect(79.), &BTreeSet::new(), o).unwrap();
        assert_eq!(if vertical { r.offset.y } else { r.offset.x }, 1.);
    }
}
#[derive(Default)]
struct Observed {
    inner: RstarIndex,
    queries: Rc<Cell<usize>>,
    changes: Rc<Cell<usize>>,
    rebuilds: Rc<Cell<usize>>,
    empty: bool,
}
impl SpatialIndex for Observed {
    fn rebuild(&mut self, items: &[(NodeIdPath, Rect)]) {
        self.rebuilds.set(self.rebuilds.get() + 1);
        self.inner.rebuild(items);
    }
    fn update(&mut self, items: &[(NodeIdPath, Rect)], removed: &[NodeIdPath]) {
        self.changes
            .set(self.changes.get() + items.len() + removed.len());
        self.inner.update(items, removed);
    }
    fn query_rect(&self, r: Rect) -> Vec<NodeIdPath> {
        self.queries.set(self.queries.get() + 1);
        if self.empty {
            vec![]
        } else {
            self.inner.query_rect(r)
        }
    }
    fn query_point(&self, x: f64, y: f64) -> Vec<NodeIdPath> {
        self.inner.query_point(x, y)
    }
}
#[test]
fn candidates_only_come_from_index_and_clean_sync_does_no_index_work() {
    let index = Observed::default();
    let q = index.queries.clone();
    let c = index.changes.clone();
    let b = index.rebuilds.clone();
    let mut s = Scene::new(index);
    let nodes = vec![node("a", None, Rect::new(10., 10., 10., 10.))];
    s.sync(&nodes).unwrap();
    let before = c.get();
    s.sync(&nodes).unwrap();
    assert_eq!(c.get(), before);
    assert_eq!(b.get(), 0);
    s.snap(Rect::new(21., 12., 5., 5.), &BTreeSet::new(), opts())
        .unwrap();
    assert!(q.get() > 0);
    let mut s = Scene::new(Observed {
        empty: true,
        ..Default::default()
    });
    s.sync(&nodes).unwrap();
    assert_eq!(
        s.snap(Rect::new(21., 12., 5., 5.), &BTreeSet::new(), opts())
            .unwrap()
            .offset,
        p(0., 0.)
    );
}
#[test]
fn space_search_is_deterministic_clear_includes_locks_and_reserved_overlays() {
    let mut a = node("a", None, Rect::new(-20., -20., 40., 40.));
    a.locked = true;
    let s = make(&[a, node("placeholder", None, Rect::new(36., 0., 20., 20.))]);
    let r = s.find_space(20., 20., Some(p(0., 0.))).unwrap();
    assert!(!r.intersects(&Rect::new(-20., -20., 40., 40.)));
    assert!(!r.intersects(&Rect::new(36., 0., 20., 20.)));
    assert_eq!(s.find_space(20., 20., Some(p(0., 0.))).unwrap(), r);
    assert_eq!(
        make(&[]).find_space(20., 30., Some(p(-100., 50.))).unwrap(),
        Rect::new(-100., 50., 20., 30.)
    );
    assert!(s.find_space(-1., 20., None).is_err());
    assert!(s.find_space(f64::NAN, 20., None).is_err());
}

#[test]
fn rounded_nested_clips_and_degenerate_boxes() {
    let mut root = node("r", None, Rect::new(0., 0., 100., 100.));
    root.clip = true;
    root.clip_radii = [50.; 4];
    let child = node("r/c", Some("r"), Rect::new(0., 0., 100., 100.));
    let mut s = make(&[root.clone(), child.clone()]);
    assert_eq!(s.hit_point(p(1., 1.), HitDepth::Deepest), Some(id("r")));
    assert_eq!(s.hit_point(p(50., 1.), HitDepth::Deepest), Some(id("r/c")));
    assert_eq!(
        s.hit_rect(
            Rect::new(0., 0., 2., 2.),
            HitDepth::Deepest,
            Marquee::Intersects
        ),
        vec![id("r")]
    );
    let mut inner = child;
    inner.bounds = Rect::new(90., 90., 20., 20.);
    inner.clip = true;
    let grand = node("r/c/g", Some("r/c"), Rect::new(-100., -100., 300., 300.));
    s.sync(&[root, inner, grand]).unwrap();
    assert_eq!(s.hit_point(p(50., 50.), HitDepth::Deepest), Some(id("r")));
    let s = make(&[
        node("line", None, Rect::new(-10., 5., 20., 0.)),
        node("point", None, Rect::new(50., 50., 0., 0.)),
    ]);
    assert_eq!(s.hit_point(p(0., 5.), HitDepth::Deepest), Some(id("line")));
    assert_eq!(s.hit_point(p(0., 5.1), HitDepth::Deepest), None);
    assert_eq!(
        s.hit_point(p(50., 50.), HitDepth::Deepest),
        Some(id("point"))
    );
}
#[test]
fn moving_ancestor_updates_descendants_and_reservations_are_not_selectable() {
    let mut nodes = vec![
        node("r", None, Rect::new(0., 0., 20., 20.)),
        node("r/c", Some("r"), Rect::new(2., 2., 4., 4.)),
    ];
    let mut s = make(&nodes);
    nodes[0].bounds.x = 100.;
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(3., 3.), HitDepth::Deepest), None);
    assert_eq!(s.hit_point(p(103., 3.), HitDepth::Deepest), Some(id("r/c")));
    assert_eq!(
        s.snap(
            Rect::new(103., 3., 4., 4.),
            &BTreeSet::from([id("r")]),
            opts()
        )
        .unwrap()
        .offset,
        p(0., 0.)
    );
    nodes[0].selectable = false;
    s.sync(&nodes).unwrap();
    assert_eq!(s.hit_point(p(103., 3.), HitDepth::Deepest), None);
    let free = s.find_space(5., 5., Some(p(103., 3.))).unwrap();
    assert!(!free.intersects(&Rect::new(100., 0., 20., 20.)));
}

#[test]
fn resolved_component_occurrences_use_layout_addresses_and_missing_layout_is_explicit() {
    use vellum_core::{
        ports::LayoutResult,
        resolver::{
            Resolver,
            libraries::{Libraries, LibrarySource},
        },
    };
    use vellum_types::{Document, Node};
    struct NoLibraries;
    impl LibrarySource for NoLibraries {
        fn load(&self, _: &str) -> Result<Option<Document>, String> {
            unreachable!()
        }
    }
    let doc:Document=serde_json::from_value(serde_json::json!({
        "meta":{"name":"TEST DATA","formatVersion":1,"createdWith":"tests"},
        "components":{"comp":{"name":"TEST DATA","root":{"type":"frame","id":"def","name":"TEST DATA","width":20,"height":20,"children":[{"type":"rect","id":"shared","name":"TEST DATA","width":5,"height":5}]}}},
        "pages":[]
    })).unwrap();
    let make_instance = |name: &str| -> Node {
        serde_json::from_value(serde_json::json!({"type":"instance","id":name,"name":"TEST DATA","componentId":"comp","width":20,"height":20})).unwrap()
    };
    let roots = Resolver::new().resolve_nodes(
        &doc,
        "/TEST DATA/file.vellum",
        &[make_instance("one"), make_instance("two")],
        &mut Libraries::new(&NoLibraries),
    );
    let mut layout = LayoutResult::default();
    fn boxes(
        r: &vellum_core::resolver::ResolvedNode,
        x: f64,
        layout: &mut LayoutResult,
        root: bool,
    ) {
        layout.boxes.insert(
            r.address.clone(),
            if root {
                Rect::new(x, 0., 20., 20.)
            } else {
                Rect::new(1., 1., 5., 5.)
            },
        );
        for c in &r.children {
            boxes(c, 0., layout, false);
        }
    }
    boxes(&roots[0], 0., &mut layout, true);
    boxes(&roots[1], 100., &mut layout, true);
    let nodes = from_resolved(&roots, &layout).unwrap();
    let s = make(&nodes);
    let a = s.hit_point(p(3., 3.), HitDepth::Deepest).unwrap();
    let b = s.hit_point(p(103., 3.), HitDepth::Deepest).unwrap();
    assert_eq!(a.last(), b.last());
    assert_ne!(a, b);
    assert_eq!(a.first(), id("one").first());
    assert_eq!(b.first(), id("two").first());
    layout.boxes.remove(&b);
    assert_eq!(
        from_resolved(&roots, &layout),
        Err(SceneError::MissingLayout(b))
    );
}

#[test]
fn spatial_incremental_queries_match_naive_over_generated_edit_sequences() {
    use proptest::prelude::*;
    use vellum_contracts::fakes::NaiveSpatialIndex;
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    });
    runner
        .run(
            &prop::collection::vec(
                (
                    0u8..40,
                    -1000i32..1000,
                    -1000i32..1000,
                    0u16..100,
                    0u16..100,
                    any::<bool>(),
                ),
                1..160,
            ),
            |edits| {
                let mut real = RstarIndex::new();
                let mut fake = NaiveSpatialIndex::new();
                for (n, x, y, w, h, remove) in edits {
                    let address = id(&format!("i{}/shared", n));
                    let r = Rect::new(x as f64, y as f64, w as f64, h as f64);
                    if remove {
                        real.update(&[], std::slice::from_ref(&address));
                        fake.update(&[], &[address]);
                    } else {
                        real.update(&[(address.clone(), r)], &[]);
                        fake.update(&[(address, r)], &[]);
                    }
                    for query in [r, r.expand(100.), Rect::new(-2000., -2000., 4000., 4000.)] {
                        let mut a = real.query_rect(query);
                        let mut b = fake.query_rect(query);
                        a.sort();
                        b.sort();
                        prop_assert_eq!(a, b);
                    }
                    let mut a = real.query_point(r.x, r.y);
                    let mut b = fake.query_point(r.x, r.y);
                    a.sort();
                    b.sort();
                    prop_assert_eq!(a, b);
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn incremental_scene_matches_fresh_after_generated_parent_changes() {
    use proptest::prelude::*;
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: 64,
        ..Default::default()
    });
    runner
        .run(
            &prop::collection::vec(
                (
                    -200i16..200,
                    -200i16..200,
                    -180i16..180,
                    any::<bool>(),
                    any::<bool>(),
                    any::<bool>(),
                ),
                1..30,
            ),
            |edits| {
                let mut nodes = vec![
                    node("r", None, Rect::new(0., 0., 100., 100.)),
                    node("r/a", Some("r"), Rect::new(-20., 10., 80., 60.)),
                    node("r/b", Some("r"), Rect::new(40., 30., 80., 40.)),
                ];
                let mut live = make(&nodes);
                for (x, y, rotation, clip, visible, locked) in edits {
                    nodes[0].bounds.x = x as f64;
                    nodes[0].bounds.y = y as f64;
                    nodes[0].rotation = rotation as f64;
                    nodes[0].clip = clip;
                    nodes[0].visible = visible;
                    nodes[0].locked = locked;
                    nodes.swap(1, 2);
                    live.sync(&nodes).unwrap();
                    let fresh = make(&nodes);
                    for (dx, dy) in [(0., 0.), (20., 20.), (70., 50.), (110., 60.)] {
                        let point = p(x as f64 + dx, y as f64 + dy);
                        let rect = Rect::new(point.x, point.y, 15., 15.);
                        for depth in [HitDepth::TopLevel, HitDepth::Deepest] {
                            prop_assert_eq!(
                                live.hit_point(point, depth),
                                fresh.hit_point(point, depth)
                            );
                            prop_assert_eq!(
                                live.hit_rect(rect, depth, Marquee::Intersects),
                                fresh.hit_rect(rect, depth, Marquee::Intersects)
                            );
                        }
                        prop_assert_eq!(live.cull(rect), fresh.cull(rect));
                    }
                }
                Ok(())
            },
        )
        .unwrap();
}

#[test]
#[ignore = "benchmark: run with --release --ignored --nocapture; no external services"]
fn baseline_5000() {
    use std::{
        hint::black_box,
        time::{Duration, Instant},
    };
    let mut nodes = (0..5000)
        .map(|n| {
            node(
                &format!("n{n}"),
                None,
                Rect::new((n % 100) as f64 * 120., (n / 100) as f64 * 100., 100., 80.),
            )
        })
        .collect::<Vec<_>>();
    let start = Instant::now();
    let mut s = make(&nodes);
    eprintln!(
        "scene baseline: 5000 nodes, initial sync {:.3} ms",
        start.elapsed().as_secs_f64() * 1000.
    );
    fn measure(name: &str, count: usize, mut op: impl FnMut(usize)) {
        let mut samples = Vec::<Duration>::with_capacity(count);
        for n in 0..count {
            let start = Instant::now();
            op(n);
            samples.push(start.elapsed());
        }
        samples.sort();
        eprintln!(
            "{name}: median {:.3} us, p95 {:.3} us ({count} samples)",
            samples[count / 2].as_secs_f64() * 1e6,
            samples[count * 95 / 100].as_secs_f64() * 1e6
        );
    }
    measure("point", 2000, |n| {
        black_box(s.hit_point(
            p((n % 100) as f64 * 120. + 10., (n % 50) as f64 * 100. + 10.),
            HitDepth::Deepest,
        ));
    });
    let view = Rect::new(0., 0., 1280., 800.);
    measure("marquee", 500, |_| {
        black_box(s.hit_rect(view, HitDepth::TopLevel, Marquee::Intersects));
    });
    measure("cull", 500, |_| {
        black_box(s.cull(view));
    });
    let mut o = opts();
    o.search = view;
    o.equal_spacing = true;
    o.centres = true;
    measure("snap", 500, |n| {
        black_box(
            s.snap(
                Rect::new((n % 10) as f64 * 120. + 103., 33., 50., 40.),
                &BTreeSet::new(),
                o,
            )
            .unwrap(),
        );
    });
    measure("find_space", 100, |_| {
        black_box(s.find_space(320., 200., None).unwrap());
    });
    measure("single-move sync", 100, |n| {
        nodes[n].bounds.x += 1.;
        s.sync(&nodes).unwrap();
    });
}

#[test]
fn tiny_nonzero_clips_are_not_empty_and_large_radii_do_not_overflow() {
    let mut root = node("r", None, Rect::new(0., 0., 1e-6, 1e-6));
    root.clip = true;
    let child = node("r/c", Some("r"), Rect::new(0., 0., 1e-6, 1e-6));
    assert_eq!(
        make(&[root, child]).hit_point(p(5e-7, 5e-7), HitDepth::Deepest),
        Some(id("r/c"))
    );
    let mut root = node("r", None, Rect::new(0., 0., 100., 100.));
    root.clip = true;
    root.clip_radii = [f64::MAX; 4];
    let child = node("r/c", Some("r"), Rect::new(0., 0., 100., 100.));
    assert_eq!(
        make(&[root, child]).hit_point(p(1., 1.), HitDepth::Deepest),
        Some(id("r"))
    );
}
