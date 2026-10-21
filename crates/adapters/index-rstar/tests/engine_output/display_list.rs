use super::common::*;
use serde_json::json;
use vellum_contracts::fakes::{FixedMetricText, StaticFonts};
use vellum_core::{
    display_list::{self, Viewport},
    ports::Rect,
    scene::Scene,
};
use vellum_index_rstar::RstarIndex;
use vellum_types::display::{DISPLAY_LIST_VERSION, DisplayList, DrawCmd, Resource};
fn build(
    roots: &[std::sync::Arc<vellum_core::resolver::ResolvedNode>],
    l: &vellum_core::ports::LayoutResult,
    viewport: Rect,
) -> display_list::Output {
    display_list::build(
        roots,
        l,
        &FixedMetricText::new(),
        &StaticFonts::new(&[]),
        &mut Scene::new(RstarIndex::new()),
        Viewport {
            bounds: viewport,
            scale: 2.,
        },
    )
    .unwrap()
}
#[test]
fn deterministic_versioned_binary_and_balanced_state() {
    let roots = resolve(vec![node(
        "f",
        "frame",
        json!({"clip":true,"opacity":0.5,"children":[node("r","rect",json!({"fills":[{"type":"solid","color":"#123456"}]}))]}),
    )]);
    let l = boxes(&[("f", [10., 20., 100., 100.]), ("f/r", [5., 7., 20., 20.])]);
    let a = build(&roots, &l, Rect::new(0., 0., 200., 200.));
    let bytes = a.list.to_bytes().unwrap();
    assert_eq!(&bytes[0..4], b"VDL\0");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        DISPLAY_LIST_VERSION
    );
    assert_eq!(
        bytes,
        build(&roots, &l, Rect::new(0., 0., 200., 200.))
            .list
            .to_bytes()
            .unwrap()
    );
    assert_eq!(DisplayList::from_bytes(&bytes).unwrap(), a.list);
    let mut depth = 0;
    for cmd in &a.list.commands {
        match cmd {
            DrawCmd::Save | DrawCmd::Layer { .. } => depth += 1,
            DrawCmd::Restore => {
                depth -= 1;
                assert!(depth >= 0)
            }
            _ => {}
        }
    }
    assert_eq!(depth, 0);
    assert!(
        a.list
            .commands
            .iter()
            .any(|c| matches!(c, DrawCmd::Clip { .. }))
    );
    assert!(
        a.list
            .commands
            .iter()
            .any(|c| matches!(c,DrawCmd::Layer{opacity,..} if *opacity==0.5))
    );
}
#[test]
fn culling_keeps_ancestors_and_children_outside_unclipped_parent() {
    let roots = resolve(vec![
        node(
            "f",
            "frame",
            json!({"children":[node("r","rect",json!({"fills":[{"type":"solid","color":"#123456"}]}))]}),
        ),
        node(
            "off",
            "rect",
            json!({"fills":[{"type":"solid","color":"#FF0000"}]}),
        ),
    ]);
    let l = boxes(&[
        ("f", [0., 0., 10., 10.]),
        ("f/r", [100., 100., 20., 20.]),
        ("off", [500., 500., 10., 10.]),
    ]);
    let out = build(&roots, &l, Rect::new(100., 100., 30., 30.));
    assert_eq!(out.painted, vec![id("f/r")]);
    assert!(
        out.list
            .commands
            .iter()
            .any(|c| matches!(c,DrawCmd::Transform(m) if m[4]==-200. && m[5]==-200.))
    );
}
#[test]
fn culling_includes_shadow_and_stroke_outside_layout_box() {
    let roots = resolve(vec![node(
        "r",
        "rect",
        json!({"fills":[{"type":"solid","color":"#123456"}],"effects":[{"type":"drop-shadow","x":50,"y":0,"blur":5,"color":"#000000"}]}),
    )]);
    let l = boxes(&[("r", [0., 0., 10., 10.])]);
    let out = build(&roots, &l, Rect::new(50., 0., 10., 10.));
    assert_eq!(out.painted, vec![id("r")]);
    assert!(
        out.list
            .commands
            .iter()
            .any(|c| matches!(c, DrawCmd::Effect { .. }))
    );
}
#[test]
fn glyph_ids_clusters_font_identity_and_styles_survive_buffer() {
    let roots = resolve(vec![node(
        "t",
        "text",
        json!({"fills":[{"type":"solid","color":"#123456"}],"content":[{"text":"ab","font":"TEST DATA Sans","size":12,"underline":true,"link":"https://example.test"}]}),
    )]);
    let out = build(
        &roots,
        &boxes(&[("t", [0., 0., 100., 100.])]),
        Rect::new(0., 0., 100., 100.),
    );
    assert!(out.list.resources.iter().any(|r| matches!(r,Resource::Glyphs(g) if g.glyphs.len()==2 && g.underline.is_some() && g.link.as_deref()==Some("https://example.test"))));
    assert_eq!(
        DisplayList::from_bytes(&out.list.to_bytes().unwrap()).unwrap(),
        out.list
    );
}
#[test]
fn malformed_buffers_and_invalid_viewports_are_refused() {
    let roots = resolve(vec![node("r", "rect", json!({}))]);
    let l = boxes(&[("r", [0., 0., 10., 10.])]);
    assert!(
        display_list::build(
            &roots,
            &l,
            &FixedMetricText::new(),
            &StaticFonts::new(&[]),
            &mut Scene::new(RstarIndex::new()),
            Viewport {
                bounds: Rect::default(),
                scale: 0.
            }
        )
        .is_err()
    );
    let mut bytes = build(&roots, &l, Rect::new(0., 0., 100., 100.))
        .list
        .to_bytes()
        .unwrap();
    for end in 0..bytes.len() {
        assert!(DisplayList::from_bytes(&bytes[..end]).is_err());
    }
    bytes[4..8].copy_from_slice(&(DISPLAY_LIST_VERSION + 1).to_le_bytes());
    assert!(DisplayList::from_bytes(&bytes).is_err());
}
#[test]
fn device_transform_preserves_small_offsets_far_from_origin() {
    let roots = resolve(vec![node(
        "r",
        "rect",
        json!({"fills":[{"type":"solid","color":"#123456"}]}),
    )]);
    let out = build(
        &roots,
        &boxes(&[("r", [1e12 + 1., 1e12 + 2., 10., 10.])]),
        Rect::new(1e12, 1e12, 100., 100.),
    );
    let mut matrix = [1., 0., 0., 1., 0., 0.];
    let mut stack = vec![];
    let mut point = None;
    for cmd in &out.list.commands {
        match cmd {
            DrawCmd::Save | DrawCmd::Layer { .. } => stack.push(matrix),
            DrawCmd::Restore => matrix = stack.pop().unwrap(),
            DrawCmd::Transform(m) => {
                matrix = *m;
            }
            DrawCmd::Path { .. } => point = Some([matrix[4], matrix[5]]),
            _ => {}
        }
    }
    assert_eq!(point, Some([2., 4.]));
}
#[test]
fn hidden_and_fully_clipped_content_never_produces_draws() {
    let roots = resolve(vec![
        node(
            "f",
            "frame",
            json!({"clip":true,"children":[node("r","rect",json!({"fills":[{"type":"solid","color":"#123456"}]}))]}),
        ),
        node(
            "hide",
            "rect",
            json!({"visible":false,"fills":[{"type":"solid","color":"#123456"}]}),
        ),
    ]);
    let out = build(
        &roots,
        &boxes(&[("f", [0., 0., 10., 10.]), ("f/r", [100., 100., 20., 20.])]),
        Rect::new(100., 100., 30., 30.),
    );
    assert!(out.painted.is_empty());
}
#[test]
fn shape_types_and_appearance_survive_with_paint_order() {
    let roots = resolve(vec![
        node(
            "v",
            "vector",
            json!({"path":"M0 0L20 0L0 20Z","viewBox":{"x":0,"y":0,"width":20,"height":20},"fills":[{"type":"solid","color":"#123456","visible":false},{"type":"gradient","kind":"linear","stops":[{"position":0,"color":"#000000"},{"position":1,"color":"#FFFFFF"}]}],"stroke":{"width":[1,2,3,4],"paints":[{"type":"solid","color":"#FF0000"}],"dash":[2,3]},"blendMode":"multiply"}),
        ),
        node(
            "i",
            "image",
            json!({"asset":"test-image","fit":"fit","crop":[1,0,0,1,0.1,0.2]}),
        ),
    ]);
    let out = build(
        &roots,
        &boxes(&[("v", [0., 0., 20., 20.]), ("i", [30., 0., 20., 20.])]),
        Rect::new(0., 0., 100., 100.),
    );
    assert_eq!(out.painted, vec![id("v"), id("i")]);
    let draws: Vec<_> = out
        .list
        .commands
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Path { stroke, .. } => Some(stroke.is_some()),
            _ => None,
        })
        .collect();
    assert_eq!(draws, vec![false, true]);
    assert_eq!(
        DisplayList::from_bytes(&out.list.to_bytes().unwrap()).unwrap(),
        out.list
    );
}
#[test]
fn boolean_operands_keep_local_transforms_and_do_not_draw_independently() {
    let roots = resolve(vec![node(
        "bool",
        "boolean",
        json!({"op":"subtract","fills":[{"type":"solid","color":"#123456"}],"children":[node("a","rect",json!({})),node("b","ellipse",json!({}))]}),
    )]);
    let out = build(
        &roots,
        &boxes(&[
            ("bool", [0., 0., 100., 100.]),
            ("bool/a", [0., 0., 100., 100.]),
            ("bool/b", [30., 40., 20., 20.]),
        ]),
        Rect::new(0., 0., 100., 100.),
    );
    assert_eq!(out.painted, vec![id("bool")]);
    assert!(out.list.resources.iter().any(|r|matches!(r,Resource::Shape(vellum_types::display::Shape::Boolean{operands,..}) if operands.len()==2 && operands[1].transform[4]==30. && operands[1].transform[5]==40.)));
}
#[test]
fn a_clipped_away_box_can_cast_a_shadow_back_inside_the_clip() {
    let roots = resolve(vec![node(
        "f",
        "frame",
        json!({"clip":true,"children":[node("r","rect",json!({"fills":[{"type":"solid","color":"#123456"}],"effects":[{"type":"drop-shadow","x":-30,"y":0,"blur":2,"color":"#000000"}]}))]}),
    )]);
    let out = build(
        &roots,
        &boxes(&[("f", [0., 0., 20., 20.]), ("f/r", [30., 0., 10., 10.])]),
        Rect::new(0., 0., 20., 20.),
    );
    assert_eq!(out.painted, vec![id("f/r")]);
}
#[test]
fn font_bytes_are_shared_and_missing_data_is_explicit() {
    use vellum_core::ports::{FontData, FontHandle, FontSource};
    struct Fonts(std::sync::Arc<[u8]>);
    impl FontSource for Fonts {
        fn resolve(&self, _: &str, _: u16, _: vellum_types::FontStyle) -> Option<FontHandle> {
            Some(FontHandle(0))
        }
        fn data(&self, h: FontHandle) -> Option<FontData> {
            (h == FontHandle(0)).then(|| FontData {
                bytes: self.0.clone(),
                index: 0,
            })
        }
    }
    let roots = resolve(vec![node(
        "t",
        "text",
        json!({"fills":[{"type":"solid","color":"#123456"}],"content":[{"text":"font bytes","font":"TEST DATA Sans","size":12}]}),
    )]);
    let l = boxes(&[("t", [0., 0., 100., 100.])]);
    let fonts = Fonts(std::sync::Arc::from([0u8, 1, 2, 3]));
    let out = display_list::build(
        &roots,
        &l,
        &FixedMetricText::new(),
        &fonts,
        &mut Scene::new(RstarIndex::new()),
        Viewport {
            bounds: Rect::new(0., 0., 100., 100.),
            scale: 1.,
        },
    )
    .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &out.fonts[&FontHandle(0)].bytes,
        &fonts.0
    ));
    assert!(out.problems.is_empty());
    let missing = build(&roots, &l, Rect::new(0., 0., 100., 100.));
    assert!(
        missing
            .problems
            .contains(&display_list::Problem::MissingFontData {
                node: id("t"),
                font: FontHandle(0)
            })
    );
}

#[test]
fn embed_uses_derived_pixels_at_device_size_without_mutating_the_node() {
    use std::collections::BTreeMap;
    use vellum_types::AssetId;
    let roots = resolve(vec![node(
        "web",
        "embed",
        json!({"url":"http://localhost:3000/","device":"phone","width":390,"height":844,"radius":12}),
    )]);
    let original = roots.clone();
    let l = boxes(&[("web", [0., 0., 390., 844.])]);
    let images = BTreeMap::from([(id("web"), AssetId::new("preview0001").unwrap())]);
    let out = display_list::build_with_embeds(
        &roots,
        &l,
        &FixedMetricText::new(),
        &StaticFonts::new(&[]),
        &mut Scene::new(RstarIndex::new()),
        Viewport {
            bounds: Rect::new(0., 0., 390., 844.),
            scale: 1.,
        },
        &images,
    )
    .unwrap();
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    assert!(
        out.list
            .commands
            .iter()
            .any(|c| matches!(c,DrawCmd::Image{dst,..} if *dst==[0.,0.,390.,844.]))
    );
    assert!(out.list.resources.iter().any(|r|matches!(r,Resource::Shape(vellum_types::display::Shape::Rect{radii,..}) if *radii==[12.;4])));
    assert_eq!(roots, original);
}
