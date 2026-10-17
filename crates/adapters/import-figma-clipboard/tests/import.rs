//! TEST DATA. `fixtures/*.html` are written by `fixtures/generate.mjs` with the reference Kiwi implementation.
use serde_json::{Value, json};
use vellum_core::ports::{ImportInput, Importer};
use vellum_import_figma_clipboard::FigmaClipboardImporter;

const SELECTION: &[u8] = include_bytes!("fixtures/selection.html");
const PAGE: &[u8] = include_bytes!("fixtures/page.html");

fn import(bytes: &[u8]) -> vellum_core::ports::Imported {
    FigmaClipboardImporter
        .import(&ImportInput { bytes, name: "" })
        .expect("imports")
}

fn kid<'a>(node: &'a Value, name: &str) -> &'a Value {
    node["children"]
        .as_array()
        .and_then(|c| c.iter().find(|n| n["name"] == name))
        .unwrap_or_else(|| panic!("no child {name} in {node}"))
}

#[test]
fn passes_the_importer_contract() {
    vellum_contracts::suites::import::importer(&FigmaClipboardImporter, SELECTION);
}

#[test]
fn the_selection_comes_back_with_its_top_left_at_the_origin() {
    let i = import(SELECTION);
    assert_eq!(i.nodes.len(), 2);
    assert_eq!(i.nodes[0]["name"], "Card");
    assert_eq!(
        i.nodes[0]["position"],
        json!({"type":"absolute","x":80.0,"y":0.0})
    );
    assert_eq!(i.nodes[1]["name"], "Loose");
    assert_eq!(
        i.nodes[1]["position"],
        json!({"type":"absolute","x":0.0,"y":350.0})
    );
    assert!(i.assets.is_empty());
}

#[test]
fn a_page_paste_is_looked_through_to_its_layers() {
    let a = import(SELECTION);
    let b = import(PAGE);
    assert_eq!(
        a.nodes, b.nodes,
        "a document and page around the layers change nothing"
    );
}

#[test]
fn a_frame_keeps_its_box_corners_stroke_shadow_and_clip() {
    let card = &import(SELECTION).nodes[0];
    assert_eq!(card["type"], "frame");
    assert_eq!(
        (card["width"].as_f64(), card["height"].as_f64()),
        (Some(320.), Some(200.))
    );
    assert_eq!(card["clip"], json!(true));
    assert_eq!(card["cornerRadius"], json!(12.0));
    assert_eq!(card["fills"], json!([{"type":"solid","color":"#FFFFFF"}]));
    assert_eq!(
        card["stroke"],
        json!({"paints":[{"type":"solid","color":"#000000"}],"width":2.0,"align":"inside"})
    );
    assert_eq!(
        card["effects"],
        json!([{"type":"drop-shadow","x":0.0,"y":4.0,"blur":8.0,"spread":0.0,"color":"#00000040"}])
    );
}

#[test]
fn children_go_back_to_front_by_position_and_unreadable_kinds_are_dropped() {
    let card = &import(SELECTION).nodes[0];
    let names: Vec<_> = card["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "Title",
            "Swatch",
            "Dot",
            "Arrow",
            "Pair",
            "Hidden",
            "Open path",
            "Button"
        ]
    );
}

#[test]
fn text_carries_its_runs_alignment_and_sizing() {
    let card = &import(SELECTION).nodes[0];
    let t = kid(card, "Title");
    assert_eq!(t["type"], "text");
    assert_eq!(
        t["content"],
        json!([{"text":"Hello Figma","font":"Inter","weight":700,"size":20.0,"lineHeight":24.0,"letterSpacing":-1.0}])
    );
    assert_eq!(t["align"], "center");
    assert_eq!(
        (
            t["width"].as_str(),
            t["height"].as_str(),
            t["resize"].as_str()
        ),
        (Some("hug"), Some("hug"), Some("auto-width"))
    );
    assert_eq!(t["fills"], json!([{"type":"solid","color":"#003366"}]));
    assert_eq!(t["position"], json!({"type":"absolute","x":16.0,"y":16.0}));
}

#[test]
fn a_rotated_layer_keeps_its_centre() {
    let card = &import(SELECTION).nodes[0];
    let s = kid(card, "Swatch");
    assert_eq!(s["type"], "rect");
    assert_eq!(s["rotation"], json!(90.0));
    // The centre is (60, 100) in the frame; the unrotated 40 by 20 box has its top left at (40, 90).
    assert_eq!(s["position"], json!({"type":"absolute","x":40.0,"y":90.0}));
    assert_eq!(
        s["fills"],
        json!([{"type":"solid","color":"#FF0000"}]),
        "the gradient's first stop stands in; the image is gone"
    );
}

#[test]
fn shapes_fills_with_opacity_and_independent_corners() {
    let card = &import(SELECTION).nodes[0];
    assert_eq!(kid(card, "Dot")["type"], "ellipse");
    assert_eq!(
        kid(card, "Dot")["fills"],
        json!([{"type":"solid","color":"#00FF0080"}])
    );
    let pair = kid(card, "Pair");
    assert_eq!(pair["type"], "group");
    let chip = kid(pair, "Chip");
    assert_eq!(chip["cornerRadius"], json!([4.0, 0.0, 8.0, 0.0]));
    let rule = kid(pair, "Rule");
    assert_eq!(
        (
            rule["type"].as_str(),
            rule["width"].as_f64(),
            rule["height"].as_f64()
        ),
        (Some("line"), Some(60.), Some(0.))
    );
    assert_eq!(
        rule["stroke"],
        json!({"paints":[{"type":"solid","color":"#666666"}],"width":1.0,"cap":"round","dash":[4.0,2.0]})
    );
}

#[test]
fn vectors_use_their_outline_and_an_outlined_stroke_is_filled_in_the_stroke_colour() {
    let card = &import(SELECTION).nodes[0];
    let arrow = kid(card, "Arrow");
    assert_eq!(arrow["type"], "vector");
    assert_eq!(
        arrow["path"],
        "M0 0 L24 0 C24 8 16 24 8 24 C2.667 24 0 20 0 12 Z"
    );
    assert_eq!(arrow["fillRule"], "evenodd");
    assert_eq!(
        arrow["viewBox"],
        json!({"x":0,"y":0,"width":24.0,"height":24.0})
    );
    let open = kid(card, "Open path");
    assert_eq!(open["path"], "M0 0 L10 10 L10 12 L0 2 Z");
    assert_eq!(open["fills"], json!([{"type":"solid","color":"#0000FF"}]));
    assert!(open.get("stroke").is_none());
}

#[test]
fn hidden_layers_stay_hidden_and_instances_become_frames_without_a_clip() {
    let card = &import(SELECTION).nodes[0];
    let hidden = kid(card, "Hidden");
    assert_eq!(
        (hidden["visible"].as_bool(), hidden["opacity"].as_f64()),
        (Some(false), Some(0.5))
    );
    let button = kid(card, "Button");
    assert_eq!(button["type"], "frame");
    assert!(button.get("clip").is_none());
}

#[test]
fn the_notes_say_what_was_left_out_and_always_say_images_are_not_in_a_paste() {
    let i = import(SELECTION);
    assert_eq!(
        i.notes,
        vec![
            "A Figma paste does not carry images: 1 image fill was left out.".to_owned(),
            "1 gradient fill became a flat colour; gradients are not imported yet.".to_owned(),
            "1 frame used auto layout; children keep their positions but no longer reflow.".to_owned(),
            "1 component or instance became plain frames.".to_owned(),
            "1 layer of a kind that is not imported (slices, stickies, connectors and so on) was left out.".to_owned(),
        ]
    );
}

#[test]
fn every_node_reads_as_a_vellum_node() {
    // Ids and names are the caller's to fill in; the rest must already be the file format.
    fn check(n: &Value) {
        assert!(n["type"].is_string(), "{n}");
        for c in n["children"].as_array().into_iter().flatten() {
            check(c);
        }
    }
    import(SELECTION).nodes.iter().for_each(check);
}

#[test]
fn damaged_and_foreign_pastes_are_refused_with_a_reason() {
    let err = |bytes: &[u8]| {
        FigmaClipboardImporter
            .import(&ImportInput { bytes, name: "" })
            .unwrap_err()
            .to_string()
    };
    assert!(err(b"<p>hello</p>").contains("not a Figma paste"));
    assert!(err(b"<span data-buffer=\"<!--(figma)!!!(/figma)-->\">").contains("base64"));
    assert!(
        err(b"<!--(figma)aGVsbG8gd29ybGQgdGhpcyBpcyBub3Qga2l3aQ==(/figma)-->").contains("fig-kiwi")
    );
    let mut cut = String::from_utf8(SELECTION.to_vec()).unwrap();
    let at = cut.find("(figma)").unwrap() + 60;
    cut.replace_range(at.., "(/figma)-->");
    assert!(
        err(cut.as_bytes()).contains("could not be read")
            || err(cut.as_bytes()).contains("damaged")
    );
}

#[test]
fn a_paste_that_is_zstd_compressed_says_so() {
    use vellum_core::base64;
    let mut file = b"fig-kiwi".to_vec();
    file.extend_from_slice(&15u32.to_le_bytes());
    for _ in 0..2 {
        file.extend_from_slice(&6u32.to_le_bytes());
        file.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd, 0, 0]);
    }
    let html = format!("<!--(figma){}(/figma)-->", base64::encode(&file));
    let e = FigmaClipboardImporter
        .import(&ImportInput {
            bytes: html.as_bytes(),
            name: "",
        })
        .unwrap_err()
        .to_string();
    assert!(e.contains("zstd"), "{e}");
}

#[test]
fn figjam_is_named_as_such() {
    use vellum_core::base64;
    let mut file = b"fig-jam.".to_vec();
    file.extend_from_slice(&[0; 8]);
    let html = format!("<!--(figma){}(/figma)-->", base64::encode(&file));
    let e = FigmaClipboardImporter
        .import(&ImportInput {
            bytes: html.as_bytes(),
            name: "",
        })
        .unwrap_err()
        .to_string();
    assert!(e.contains("FigJam"), "{e}");
}

#[test]
fn recognizes_a_paste_by_its_marker() {
    let r = |b: &[u8]| FigmaClipboardImporter.recognizes(&ImportInput { bytes: b, name: "" });
    assert!(r(SELECTION));
    assert!(!r(b"<svg/>"));
}
