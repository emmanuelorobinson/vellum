//! TEST DATA: shared sizing semantics and conversion of addressed resolver output.
use vellum_core::{
    layout::{self, Axis, LayoutError},
    ports::*,
    resolver::{
        Resolver,
        libraries::{Libraries, LibrarySource},
    },
};
use vellum_types::{Document, Layout, Node, NodeIdPath};
fn id(s: &str) -> NodeIdPath {
    s.parse().unwrap()
}
fn bare(w: Dimension) -> LayoutTree {
    LayoutTree {
        root: LayoutNode {
            id: id("r"),
            style: LayoutStyle {
                width: w,
                height: Dimension::Fixed(20.0),
                ..Default::default()
            },
            content: LayoutContent::Frame,
            children: vec![],
        },
    }
}
#[test]
fn rejects_missing_fallback_invalid_numbers_grid_and_duplicate_addresses() {
    assert!(matches!(
        layout::prepare(&bare(Dimension::Fill { fallback: None })),
        Err(LayoutError::MissingFallback {
            axis: Axis::Width,
            ..
        })
    ));
    for x in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
        assert!(matches!(
            layout::prepare(&bare(Dimension::Fixed(x))),
            Err(LayoutError::InvalidNumber { .. })
        ));
    }
    let mut t = bare(Dimension::Fixed(100.0));
    t.root.style.mode = Layout::Grid;
    assert!(matches!(
        layout::prepare(&t),
        Err(LayoutError::InvalidGrid { .. })
    ));
    t.root.style.mode = Layout::None;
    t.root.children.push(t.root.clone());
    assert!(matches!(
        layout::prepare(&t),
        Err(LayoutError::DuplicateAddress { .. })
    ));
}
struct NoLibraries;
impl LibrarySource for NoLibraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        unreachable!()
    }
}
#[test]
fn resolved_tree_preserves_occurrences_numbers_text_strokes_and_positions() {
    let doc = Document::new("TEST DATA");
    let node: Node = serde_json::from_value(serde_json::json!({"type":"frame","id":"r","name":"TEST DATA","width":100,"height":100,"layout":"row","layoutIncludesStroke":true,"padding":[2,4],"stroke":{"paints":[],"width":[2,4,6,8],"align":"center"},"children":[{"type":"text","id":"t","name":"TEST DATA","width":"hug","height":"hug","position":{"type":"absolute","x":-3,"y":5},"content":[{"text":"TEST DATA","font":"TEST DATA Sans","size":12}]}]})).unwrap();
    let resolved = Resolver::new().resolve_node(
        &doc,
        "/TEST DATA/doc.vellum",
        &node,
        &mut Libraries::new(&NoLibraries),
    );
    let tree = layout::from_resolved(&resolved).unwrap();
    assert_eq!(tree.root.id, resolved.address);
    assert_eq!(
        tree.root.style.padding,
        Insets {
            top: 2.0,
            right: 4.0,
            bottom: 2.0,
            left: 4.0
        }
    );
    assert_eq!(
        tree.root.style.stroke,
        Insets {
            top: 1.0,
            right: 2.0,
            bottom: 3.0,
            left: 4.0
        }
    );
    assert!(tree.root.children[0].style.absolute);
    assert_eq!(tree.root.children[0].style.x, -3.0);
    assert!(
        matches!(&tree.root.children[0].content,LayoutContent::Text(t) if t.runs[0].size==12.0)
    );
    assert!(layout::prepare(&tree).is_ok());
}

#[test]
fn empty_hugging_frames_use_fallback_but_content_beats_it() {
    let mut t = bare(Dimension::Hug {
        fallback: Some(73.0),
    });
    t.root.style.mode = Layout::Row;
    assert_eq!(
        layout::prepare(&t).unwrap().root.width,
        layout::Sizing::Fixed(73.0)
    );
    let mut c = bare(Dimension::Fixed(25.0)).root;
    c.id = id("c");
    t.root.children.push(c);
    assert_eq!(layout::prepare(&t).unwrap().root.width, layout::Sizing::Hug);
    t.root.children[0].style.visible = false;
    assert_eq!(
        layout::prepare(&t).unwrap().root.width,
        layout::Sizing::Fixed(73.0)
    );
}
#[test]
fn repeated_component_nodes_keep_separate_layout_addresses() {
    let mut doc = Document::new("TEST DATA");
    let component=serde_json::from_value(serde_json::json!({"name":"TEST DATA","root":{"type":"frame","id":"definition","name":"TEST DATA","width":40,"height":20,"layout":"row","children":[{"type":"rect","id":"same","name":"TEST DATA","width":20,"height":10}]}})).unwrap();
    doc.components
        .insert(vellum_types::ComponentId::new("c").unwrap(), component);
    let root:Node=serde_json::from_value(serde_json::json!({"type":"frame","id":"r","name":"TEST DATA","width":200,"height":100,"layout":"row","children":[{"type":"instance","id":"one","name":"TEST DATA","componentId":"c","width":40,"height":20},{"type":"instance","id":"two","name":"TEST DATA","componentId":"c","width":60,"height":20}]})).unwrap();
    let resolved = Resolver::new().resolve_node(
        &doc,
        "/TEST DATA/doc.vellum",
        &root,
        &mut Libraries::new(&NoLibraries),
    );
    let tree = layout::from_resolved(&resolved).unwrap();
    assert_ne!(
        tree.root.children[0].children[0].id,
        tree.root.children[1].children[0].id
    );
    assert_eq!(tree.root.children[0].style.width, Dimension::Fixed(40.0));
    assert_eq!(tree.root.children[1].style.width, Dimension::Fixed(60.0));
    assert!(layout::prepare(&tree).is_ok());
}

#[test]
fn invalid_text_numbers_are_reported_before_measurement() {
    let mut t = bare(Dimension::Fixed(100.0));
    let mut rich = RichText::plain("TEST DATA", "TEST DATA Sans", f64::NAN);
    t.root.content = LayoutContent::Text(rich.clone());
    assert!(matches!(
        layout::prepare(&t),
        Err(LayoutError::InvalidNumber {
            property: "font size",
            ..
        })
    ));
    rich.runs[0].size = 12.0;
    rich.runs[0].line_height = Some(f64::INFINITY);
    t.root.content = LayoutContent::Text(rich);
    assert!(matches!(
        layout::prepare(&t),
        Err(LayoutError::InvalidNumber {
            property: "line height",
            ..
        })
    ));
}

#[test]
fn ignored_theme_choice_does_not_block_resolved_geometry() {
    let doc = Document::new("TEST DATA");
    let root:Node=serde_json::from_value(serde_json::json!({"type":"frame","id":"r","name":"TEST DATA","width":100,"height":80,"theme":{"Unknown":"Ignored"}})).unwrap();
    let resolved = Resolver::new().resolve_node(
        &doc,
        "/TEST DATA/doc.vellum",
        &root,
        &mut Libraries::new(&NoLibraries),
    );
    assert!(!resolved.problems.is_empty());
    assert!(layout::from_resolved(&resolved).is_ok());
}

#[test]
fn text_conversion_keeps_resize_vertical_alignment_links_and_decorations() {
    use vellum_types::{TextResize,VerticalAlign};
    for (resize,w,h) in [("auto-width",Dimension::Hug {fallback:None},Dimension::Hug {fallback:None}),("auto-height",Dimension::Fixed(100.0),Dimension::Hug {fallback:None}),("fixed",Dimension::Fixed(100.0),Dimension::Fixed(50.0))] {
        let doc=Document::new("TEST DATA");
        let n:Node=serde_json::from_value(serde_json::json!({"type":"text","id":"t","name":"TEST DATA","width":100,"height":50,"resize":resize,"verticalAlign":"bottom","content":[{"text":"TEST DATA","font":"Inter","size":12,"underline":true,"strikethrough":true,"link":"https://example.test"}]})).unwrap();
        let r=Resolver::new().resolve_node(&doc,"/TEST DATA/doc.vellum",&n,&mut Libraries::new(&NoLibraries));
        let t=layout::from_resolved(&r).unwrap();
        assert_eq!(t.root.style.width,w); assert_eq!(t.root.style.height,h);
        let LayoutContent::Text(rich)=t.root.content else {panic!()};
        assert_eq!(rich.vertical_align,VerticalAlign::Bottom);
        assert_eq!(rich.resize, match resize {"auto-width"=>TextResize::AutoWidth,"auto-height"=>TextResize::AutoHeight,_=>TextResize::Fixed});
        assert!(rich.runs[0].underline && rich.runs[0].strikethrough);
        assert_eq!(rich.runs[0].link.as_deref(),Some("https://example.test"));
    }
}
