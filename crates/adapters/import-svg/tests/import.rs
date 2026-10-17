//! TEST DATA. Small SVGs written for these tests; none is copied from a design tool or a library.
use serde_json::{Value, json};
use vellum_core::base64;
use vellum_core::ports::{ImportInput, Imported, Importer, asset_ref};
use vellum_import_svg::SvgImporter;

const ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
 stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
 <path d="M5 12h14"/><circle cx="12" cy="12" r="9"/></svg>"#;

fn import(svg: &str) -> Imported {
    SvgImporter
        .import(&ImportInput {
            bytes: svg.as_bytes(),
            name: "TEST DATA.svg",
        })
        .expect("imports")
}

fn children(i: &Imported) -> &Vec<Value> {
    i.nodes[0]["children"]
        .as_array()
        .expect("a frame with children")
}

#[test]
fn passes_the_importer_contract() {
    vellum_contracts::suites::import::importer(&SvgImporter, ICON.as_bytes());
}

#[test]
fn an_icon_becomes_a_frame_of_a_vector_and_an_ellipse_with_the_strokes_kept() {
    let i = import(ICON);
    let frame = &i.nodes[0];
    assert_eq!(
        (frame["type"].as_str(), frame["name"].as_str()),
        (Some("frame"), Some("TEST DATA"))
    );
    assert_eq!(
        (frame["width"].as_f64(), frame["height"].as_f64()),
        (Some(24.), Some(24.))
    );
    assert_eq!(frame["clip"], json!(true));
    let kids = children(&i);
    assert_eq!(kids.len(), 2);
    let line = &kids[0];
    assert_eq!(line["type"], "vector");
    assert_eq!(line["path"], "M5 12 L19 12");
    assert_eq!(
        line["position"],
        json!({"type":"absolute","x":5.0,"y":12.0})
    );
    assert_eq!(
        (line["width"].as_f64(), line["height"].as_f64()),
        (Some(14.), Some(1.)),
        "a flat line gets a 1 unit box"
    );
    assert_eq!(
        line["stroke"],
        json!({"paints":[{"type":"solid","color":"#000000"}],"width":2.0,"cap":"round","join":"round"})
    );
    assert!(line.get("fills").is_none(), "fill none gives no fill");
    let ring = &kids[1];
    assert_eq!(ring["type"], "ellipse");
    assert_eq!(
        (ring["width"].as_f64(), ring["height"].as_f64()),
        (Some(18.), Some(18.))
    );
    assert_eq!(ring["position"], json!({"type":"absolute","x":3.0,"y":3.0}));
    assert!(i.notes.is_empty(), "{:?}", i.notes);
}

#[test]
fn the_view_box_scales_and_centres_into_the_given_size() {
    let i = import(
        r##"<svg width="200" height="100" viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="#f00"/></svg>"##,
    );
    let rect = &children(&i)[0];
    assert_eq!(rect["type"], "rect");
    // Uniform scale 10, centred horizontally.
    assert_eq!(
        (rect["width"].as_f64(), rect["height"].as_f64()),
        (Some(100.), Some(100.))
    );
    assert_eq!(rect["position"]["x"], json!(50.0));
    assert_eq!(rect["fills"], json!([{"type":"solid","color":"#FF0000"}]));
    assert_eq!(i.nodes[0]["width"], json!(200.0));
}

#[test]
fn units_percentages_and_a_missing_size_are_understood() {
    let i =
        import(r#"<svg width="1in" viewBox="0 0 50 25"><rect width="100%" height="50%"/></svg>"#);
    assert_eq!(i.nodes[0]["width"], json!(96.0));
    assert_eq!(i.nodes[0]["height"], json!(48.0));
    let none = import(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect x="5" y="5" width="30" height="20"/></svg>"#,
    );
    assert_eq!(
        (
            none.nodes[0]["width"].as_f64(),
            none.nodes[0]["height"].as_f64()
        ),
        (Some(35.), Some(25.))
    );
}

#[test]
fn group_transforms_opacity_and_inherited_style_reach_each_shape() {
    let i = import(
        r##"<svg width="100" height="100"><g transform="translate(10 20) scale(2)" fill="#00f" opacity=".5" stroke="#000" stroke-width="1">
        <rect x="1" y="1" width="5" height="5" rx="1"/></g></svg>"##,
    );
    let r = &children(&i)[0];
    assert_eq!(r["position"], json!({"type":"absolute","x":12.0,"y":22.0}));
    assert_eq!(
        (r["width"].as_f64(), r["height"].as_f64()),
        (Some(10.), Some(10.))
    );
    assert_eq!(r["cornerRadius"], json!(2.0));
    assert_eq!(r["opacity"], json!(0.5));
    assert_eq!(
        r["stroke"]["width"],
        json!(2.0),
        "stroke widths scale with the transform"
    );
}

#[test]
fn rotation_turns_shapes_into_vectors_with_the_rotation_baked_in() {
    let i = import(
        r#"<svg width="40" height="40"><rect width="10" height="20" transform="rotate(90)"/></svg>"#,
    );
    let v = &children(&i)[0];
    assert_eq!(v["type"], "vector");
    assert_eq!(
        (v["width"].as_f64(), v["height"].as_f64()),
        (Some(20.), Some(10.))
    );
    assert_eq!(v["position"], json!({"type":"absolute","x":-20.0,"y":0.0}));
}

#[test]
fn style_attributes_classes_and_ids_cascade_in_order() {
    let i = import(
        r##"<svg width="10" height="10"><style>.a{fill:#0f0;stroke:#00f} #x{stroke:none}</style>
        <rect id="x" class="a" width="4" height="4" fill="#f00" style="fill-opacity:.5"/></svg>"##,
    );
    let r = &children(&i)[0];
    assert_eq!(
        r["fills"],
        json!([{"type":"solid","color":"#00FF0080"}]),
        "the sheet beats the attribute, the style attribute beats both"
    );
    assert!(r.get("stroke").is_none());
    assert_eq!(r["name"], "x");
}

#[test]
fn polygons_lines_and_odd_fill_rules_and_dashes() {
    let i = import(
        r#"<svg width="20" height="20"><polygon points="0,0 10,0 5,8" fill-rule="evenodd"/>
        <line x1="0" y1="10" x2="20" y2="10" stroke="red" stroke-dasharray="4 2 1"/></svg>"#,
    );
    let k = children(&i);
    assert_eq!(k[0]["path"], "M0 0 L10 0 L5 8 Z");
    assert_eq!(k[0]["fillRule"], "evenodd");
    assert_eq!(
        k[1]["stroke"]["dash"],
        json!([4.0, 2.0, 1.0, 4.0, 2.0, 1.0]),
        "an odd dash list repeats"
    );
}

#[test]
fn text_becomes_a_text_layer_with_font_size_and_weight() {
    let i = import(
        r##"<svg width="100" height="40"><text x="50" y="30" font-family="'Fira Sans', sans-serif" font-size="20"
        font-weight="bold" text-anchor="middle" fill="#123">Hello <tspan>there</tspan></text></svg>"##,
    );
    let t = &children(&i)[0];
    assert_eq!(t["type"], "text");
    assert_eq!(
        t["content"],
        json!([{"text":"Hello there","font":"Fira Sans","weight":700,"size":20.0}])
    );
    assert_eq!(t["align"], "center");
    assert_eq!(t["fills"], json!([{"type":"solid","color":"#112233"}]));
    assert_eq!(t["position"]["y"], json!(14.0));
}

#[test]
fn an_embedded_image_is_stored_and_a_linked_one_is_noted() {
    let png = {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&[0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0]);
        b
    };
    let svg = format!(
        r#"<svg width="20" height="20"><image x="2" y="3" width="10" height="8" href="data:image/png;base64,{}"/>
        <image width="5" height="5" xlink:href="https://example.com/a.png"/></svg>"#,
        base64::encode(&png)
    );
    let i = import(&svg);
    assert_eq!(i.assets.len(), 1);
    assert_eq!(
        (i.assets[0].mime.as_str(), &i.assets[0].bytes),
        ("image/png", &png)
    );
    let img = &children(&i)[0];
    assert_eq!(
        (img["type"].as_str(), img["asset"].as_str()),
        (Some("image"), Some(asset_ref(0).as_str()))
    );
    assert_eq!(img["fit"], "fit");
    assert_eq!(
        i.notes,
        vec!["1 linked image was left out; only embedded images are imported.".to_owned()]
    );
}

#[test]
fn use_references_draw_the_target_translated_and_loops_end() {
    let i = import(
        r##"<svg width="30" height="10"><defs><rect id="dot" width="4" height="4"/><use id="loop" href="#loop"/></defs>
        <use href="#dot" x="10" y="2"/><use href="#loop"/></svg>"##,
    );
    let k = children(&i);
    assert_eq!(k.len(), 1);
    assert_eq!(
        k[0]["position"],
        json!({"type":"absolute","x":10.0,"y":2.0})
    );
}

#[test]
fn what_cannot_be_represented_is_counted_not_hidden() {
    let i = import(
        r##"<svg width="10" height="10"><defs><linearGradient id="g"><stop offset="0" stop-color="#f00" stop-opacity=".5"/>
        <stop offset="1" stop-color="#00f"/></linearGradient><clipPath id="c"><rect width="1" height="1"/></clipPath></defs>
        <rect width="5" height="5" fill="url(#g)" clip-path="url(#c)"/><rect width="2" height="2" fill="url(#missing) #0f0" transform="wobble"/></svg>"##,
    );
    let k = children(&i);
    assert_eq!(
        k[0]["fills"],
        json!([{"type":"solid","color":"#FF000080"}]),
        "the first stop stands in for the gradient"
    );
    assert_eq!(
        k[1]["fills"],
        json!([{"type":"solid","color":"#00FF00"}]),
        "a missing reference uses its fallback"
    );
    assert_eq!(i.notes.len(), 3, "{:?}", i.notes);
    assert!(
        i.notes
            .iter()
            .any(|n| n.contains("2 gradient or pattern fills"))
    );
    assert!(i.notes.iter().any(|n| n.contains("clip path")));
    assert!(i.notes.iter().any(|n| n.contains("transform")));
}

#[test]
fn hidden_things_and_empty_shapes_draw_nothing() {
    let i = import(
        r#"<svg width="10" height="10"><g display="none"><rect width="3" height="3"/></g>
        <rect width="3" height="3" visibility="hidden"/><rect width="0" height="3"/><circle r="0"/><rect id="shown" width="2" height="2"/></svg>"#,
    );
    assert_eq!(children(&i).len(), 1);
    assert_eq!(children(&i)[0]["name"], "shown");
}

#[test]
fn bad_input_is_refused_with_a_sentence_and_hostile_input_is_bounded() {
    let err = |bytes: &[u8]| {
        SvgImporter
            .import(&ImportInput {
                bytes,
                name: "x.svg",
            })
            .unwrap_err()
            .to_string()
    };
    assert!(err(b"<html></html>").contains("not <svg>"));
    assert!(err(b"<svg><g></svg>").contains("could not be read"));
    assert!(err(b"\xff\xfe").contains("UTF-8"));
    assert!(err(b"<svg width='10' height='10'/>").contains("draws nothing"));
    assert!(
        err(b"<svg width='0' height='0' viewBox='0 0 0 0'><rect width='1' height='1'/></svg>")
            .contains("usable size")
    );
    let big = vec![b' '; 5 * 1024 * 1024];
    assert!(err(&big).contains("KB"));
    let bomb = "<!DOCTYPE svg [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;\">]><svg width='9' height='9'><text y='5'>&b;</text></svg>";
    let t = import(bomb);
    assert_eq!(
        children(&t)[0]["content"][0]["text"],
        "&b;",
        "entities are never expanded"
    );
    let many = format!(
        "<svg width='10' height='10'>{}</svg>",
        "<rect width='1' height='1'/>".repeat(6000)
    );
    let capped = import(&many);
    assert_eq!(children(&capped).len(), 5000);
    assert!(capped.notes.iter().any(|n| n.contains("first 5000")));
}

#[test]
fn recognizes_svg_by_its_start_and_nothing_else() {
    let r = |b: &[u8]| SvgImporter.recognizes(&ImportInput { bytes: b, name: "" });
    assert!(r(b"<?xml version='1.0'?>\n<svg xmlns='x'/>"));
    assert!(!r(b"<html><body>not svg"));
    assert!(!r(b"\x89PNG\0<svg"));
}
