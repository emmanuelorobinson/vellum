//! The bytes of a `.vellum` file (guide Prompt 9, Invariant 5): one spelling for every document. The layout is
//! fixed (2-space indent, LF, trailing newline), keys come out in the order the type declares them, maps come
//! out sorted, and numbers are rounded in the codec and nowhere else.

use serde_json::{Value as Json, json};
use vellum_codec_json::{CodecError, JsonCodec, Quantum, decode, encode, quantize};
use vellum_types::{Document, brand};

fn meta() -> Json {
    json!({"formatVersion": 1, "name": "TEST DATA", "createdWith": "Vellum 0.1.0"})
}

fn document(input: Json) -> Document {
    serde_json::from_value(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"))
}

fn bytes(doc: &Document) -> String {
    String::from_utf8(encode(doc).expect("encode")).expect("utf-8")
}

/// The file's bytes read back as JSON, so a test can look at one number.
fn printed(doc: &Document) -> Json {
    serde_json::from_slice(&encode(doc).expect("encode")).expect("the output is JSON")
}

/// A page holding one node, built from `node`.
fn with_node(node: Json) -> Document {
    document(json!({
        "meta": meta(),
        "pages": [{"id": "page000001", "name": "Canvas", "children": [node]}]
    }))
}

fn node_of(doc: &Document) -> Json {
    printed(doc)["pages"][0]["children"][0].clone()
}

// --- layout -------------------------------------------------------------------------------------

#[test]
fn a_new_document_prints_in_the_canonical_layout() {
    let expected = format!(
        "{{\n  \"meta\": {{\n    \"formatVersion\": 1,\n    \"name\": \"Launch\",\n    \"createdWith\": \"{} {}\"\n  }},\n  \"pages\": []\n}}\n",
        brand::NAME,
        env!("CARGO_PKG_VERSION"),
    );
    assert_eq!(bytes(&Document::new("Launch")), expected);
}

#[test]
fn a_file_ends_in_one_line_feed_and_holds_no_carriage_return_or_tab() {
    let doc = with_node(
        json!({"type": "rect", "id": "r0000001", "name": "Back\tline\r\nbreak",
        "width": 10.0, "height": 10.0}),
    );
    let text = bytes(&doc);
    assert!(text.ends_with("}\n"), "{text:?}");
    assert!(!text.ends_with("\n\n"));
    assert!(
        !text.contains('\r'),
        "a carriage return in a string is escaped"
    );
    assert!(!text.contains('\t'), "a tab in a string is escaped");
    assert!(text.contains("Back\\tline\\r\\nbreak"));
    for line in text.lines() {
        let indent = line.len() - line.trim_start_matches(' ').len();
        assert_eq!(indent % 2, 0, "indent is two spaces a level: {line:?}");
    }
}

#[test]
fn keys_come_out_in_the_order_the_type_declares_them() {
    // Fed in the opposite order on purpose.
    let doc = with_node(json!({
        "height": 20.0, "width": 10.0, "name": "Back", "id": "r0000001", "type": "rect"
    }));
    let text = bytes(&doc);
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {text}"))
    };
    assert!(at("\"meta\"") < at("\"pages\""));
    assert!(at("\"type\": \"rect\"") < at("\"id\": \"r0000001\""));
    assert!(at("\"id\": \"r0000001\"") < at("\"name\": \"Back\""));
    assert!(at("\"name\": \"Back\"") < at("\"width\""));
    assert!(at("\"width\"") < at("\"height\""));
}

#[test]
fn maps_come_out_sorted_by_key_whatever_order_they_were_filled_in() {
    let mut forward = Document::new("TEST DATA");
    let mut backward = Document::new("TEST DATA");
    let names = ["system", "brand", "icons", "motion"];
    for name in names {
        forward
            .imports
            .insert(name.to_owned(), format!("{name}.lib.vellum"));
    }
    for name in names.iter().rev() {
        backward
            .imports
            .insert((*name).to_owned(), format!("{name}.lib.vellum"));
    }
    let text = bytes(&forward);
    assert_eq!(text, bytes(&backward));
    let order: Vec<usize> = ["brand", "icons", "motion", "system"]
        .iter()
        .map(|name| text.find(&format!("\"{name}\":")).expect("present"))
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
}

#[test]
fn free_form_metadata_prints_with_sorted_keys_and_keeps_its_arrays_in_order() {
    let doc = with_node(json!({
        "type": "rect", "id": "r0000001", "name": "Back", "width": 10.0, "height": 10.0,
        "metadata": {"zeta": 1, "alpha": [3, 1, 2], "mid": {"b": true, "a": null}}
    }));
    let text = bytes(&doc);
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {text}"))
    };
    assert!(at("\"alpha\"") < at("\"mid\"") && at("\"mid\"") < at("\"zeta\""));
    assert!(at("\"a\": null") < at("\"b\": true"));
    assert_eq!(
        printed(&doc)["pages"][0]["children"][0]["metadata"]["alpha"],
        json!([3, 1, 2])
    );
}

#[test]
fn text_is_written_as_utf_8_with_only_the_required_escapes() {
    let doc = with_node(json!({
        "type": "text", "id": "t0000001", "name": "Quote \" and \\ and \u{1} and é and 日本 and 🙂",
        "width": "hug", "height": "hug",
        "content": [{"text": "a\u{2028}b", "font": "Geist", "size": 14.0}]
    }));
    let text = bytes(&doc);
    assert!(
        text.contains("Quote \\\" and \\\\ and \\u0001 and é and 日本 and 🙂"),
        "{text}"
    );
    assert_eq!(decode(text.as_bytes()).expect("decode"), doc);
}

// --- numbers ------------------------------------------------------------------------------------

#[test]
fn a_whole_number_has_no_fraction_and_no_exponent() {
    let doc = with_node(json!({"type": "rect", "id": "r0000001", "name": "Back",
        "width": 320.0, "height": 1.0e3, "rotation": 90.0}));
    let text = bytes(&doc);
    assert!(text.contains("\"width\": 320,"), "{text}");
    assert!(text.contains("\"height\": 1000\n"), "{text}");
    assert!(text.contains("\"rotation\": 90,"), "{text}");
    assert!(!text.contains("e3") && !text.contains("320.0") && !text.contains("1000.0"));
}

#[test]
fn negative_zero_prints_as_zero() {
    let doc = with_node(json!({"type": "rect", "id": "r0000001", "name": "Back",
        "position": {"type": "absolute", "x": -0.001, "y": -0.0}, "width": 10.0, "height": 10.0}));
    let position = &node_of(&doc)["position"];
    assert_eq!(position["x"].to_string(), "0");
    assert_eq!(position["y"].to_string(), "0");
}

#[test]
fn positions_sizes_radii_gaps_and_paddings_round_to_a_hundredth_of_a_pixel() {
    let doc = with_node(json!({
        "type": "frame", "id": "f0000001", "name": "Card",
        "position": {"type": "absolute", "x": 10.006, "y": -3.456},
        "width": 100.126, "height": {"hug": 40.004},
        "layout": "row", "gap": 1.006, "padding": [1.234, 5.678],
        "cornerRadius": [1.111, 2.226, 3.333, 4.444]
    }));
    let node = node_of(&doc);
    assert_eq!(node["position"]["x"], json!(10.01));
    assert_eq!(node["position"]["y"], json!(-3.46));
    assert_eq!(node["width"], json!(100.13));
    assert_eq!(node["height"], json!({"hug": 40}));
    assert_eq!(node["gap"], json!(1.01));
    assert_eq!(node["padding"], json!([1.23, 5.68]));
    assert_eq!(node["cornerRadius"], json!([1.11, 2.23, 3.33, 4.44]));
}

#[test]
fn ratios_opacities_and_gradient_stops_round_to_a_thousandth() {
    let doc = with_node(json!({
        "type": "ellipse", "id": "e0000001", "name": "Dial", "width": 10.0, "height": 10.0,
        "opacity": 0.12349, "innerRadius": 0.55555,
        "fills": [
            {"type": "gradient", "kind": "linear",
             "stops": [{"color": "#000000", "position": 0.12345}, {"color": "#FFFFFF", "position": 1.0}],
             "transform": [0.12345, 0.0, 0.0, 0.98765, 0.5556, 0.4444]},
            {"type": "image", "asset": "a0000001", "opacity": 0.55555,
             "crop": [1.00049, 0.0, 0.0, 1.00051, 0.0004, 0.0006]}
        ]
    }));
    let node = node_of(&doc);
    assert_eq!(node["opacity"], json!(0.123));
    assert_eq!(node["innerRadius"], json!(0.556));
    assert_eq!(node["fills"][0]["stops"][0]["position"], json!(0.123));
    assert_eq!(
        node["fills"][0]["transform"],
        json!([0.123, 0, 0, 0.988, 0.556, 0.444])
    );
    assert_eq!(node["fills"][1]["opacity"], json!(0.556));
    assert_eq!(node["fills"][1]["crop"], json!([1, 0, 0, 1.001, 0, 0.001]));
}

#[test]
fn angles_round_to_a_hundredth_of_a_degree() {
    let doc = with_node(json!({
        "type": "ellipse", "id": "e0000001", "name": "Dial", "width": 10.0, "height": 10.0,
        "rotation": 12.3456, "startAngle": 0.004, "sweepAngle": 270.0049
    }));
    let node = node_of(&doc);
    assert_eq!(node["rotation"], json!(12.35));
    assert!(
        node.get("startAngle").is_none(),
        "a value that rounds to its default is left out, as a default is: {node}"
    );
    assert_eq!(node["sweepAngle"], json!(270));
}

#[test]
fn mesh_points_and_handles_are_ratios_not_pixels() {
    let doc = with_node(json!({
        "type": "rect", "id": "r0000001", "name": "Back", "width": 10.0, "height": 10.0,
        "fills": [{"type": "mesh", "columns": 1, "rows": 1, "points": [{
            "x": 0.12345, "y": 0.98765, "color": "#FF0000",
            "handles": {"left": {"x": -0.12345, "y": 0.0}, "right": {"x": 0.12345, "y": 0.0},
                        "up": {"x": 0.0, "y": -0.12345}, "down": {"x": 0.0, "y": 0.12345}}
        }]}]
    }));
    let point = &node_of(&doc)["fills"][0]["points"][0];
    assert_eq!(point["x"], json!(0.123));
    assert_eq!(point["y"], json!(0.988));
    assert_eq!(point["handles"]["left"]["x"], json!(-0.123));
    assert_eq!(point["handles"]["down"]["y"], json!(0.123));
}

#[test]
fn strokes_effects_text_and_view_boxes_round_to_a_hundredth() {
    let doc = with_node(json!({
        "type": "vector", "id": "v0000001", "name": "Mark", "width": 24.0, "height": 24.0,
        "path": "M0 0L24 24", "viewBox": {"x": 0.004, "y": 0.0, "width": 24.006, "height": 24.0},
        "stroke": {"paints": [], "width": [1.006, 2.006, 3.007, 4.008], "dash": [2.346, 1.006]},
        "effects": [
            {"type": "drop-shadow", "x": 1.234, "y": 2.346, "blur": 3.456, "spread": 4.567, "color": "#00000040"},
            {"type": "layer-blur", "radius": 5.678}
        ]
    }));
    let node = node_of(&doc);
    assert_eq!(node["viewBox"]["width"], json!(24.01));
    assert_eq!(node["stroke"]["width"], json!([1.01, 2.01, 3.01, 4.01]));
    assert_eq!(node["stroke"]["dash"], json!([2.35, 1.01]));
    assert_eq!(node["effects"][0]["x"], json!(1.23));
    assert_eq!(node["effects"][0]["y"], json!(2.35));
    assert_eq!(node["effects"][0]["blur"], json!(3.46));
    assert_eq!(node["effects"][0]["spread"], json!(4.57));
    assert_eq!(node["effects"][1]["radius"], json!(5.68));

    let text = with_node(json!({
        "type": "text", "id": "t0000001", "name": "Title", "width": "hug", "height": "hug",
        "content": [{"text": "Hi", "font": "Geist", "size": 14.004, "lineHeight": 20.006, "letterSpacing": -0.123}]
    }));
    let run = &node_of(&text)["content"][0];
    assert_eq!(run["size"], json!(14));
    assert_eq!(run["lineHeight"], json!(20.01));
    assert_eq!(run["letterSpacing"], json!(-0.12));
}

#[test]
fn numbers_the_format_does_not_define_a_unit_for_are_written_as_they_are() {
    let doc = document(json!({
        "meta": meta(),
        "variables": {"va000001": {"type": "number", "name": "space/odd", "values": [{"value": 1.23456789}]}},
        "components": {"comp000001": {
            "name": "Counter",
            "props": {"count": {"type": "number", "default": 3.14159266}},
            "root": {"type": "frame", "id": "btn000001", "name": "Counter", "width": 40.126, "height": "hug"}
        }},
        "pages": [{"id": "page000001", "name": "Canvas", "children": [
            {"type": "instance", "id": "i0000001", "name": "Counter", "width": "hug", "height": "hug",
             "componentId": "comp000001", "props": {"count": 2.71828999, "width": 9.87654321},
             "metadata": {"width": 1.23456, "nested": {"x": 2.34567}}},
            {"type": "script", "id": "s0000001", "name": "Chart", "width": 10.0, "height": 10.0,
             "src": "chart.js", "inputs": {"scale": 0.123456789}},
            {"type": "rect", "id": "r0000001", "name": "Shader", "width": 10.0, "height": 10.0,
             "fills": [{"type": "shader", "src": "glow.sksl", "uniforms": {"k": 0.123456789, "v": [0.12345, 0.98765]}}]}
        ]}]
    }));
    let json = printed(&doc);
    assert_eq!(
        json["variables"]["va000001"]["values"][0]["value"],
        json!(1.23456789)
    );
    assert_eq!(
        json["components"]["comp000001"]["props"]["count"]["default"],
        json!(3.14159266)
    );
    assert_eq!(
        json["components"]["comp000001"]["root"]["width"],
        json!(40.13),
        "the tree is a tree"
    );
    let instance = &json["pages"][0]["children"][0];
    assert_eq!(
        instance["props"],
        json!({"count": 2.71828999, "width": 9.87654321})
    );
    assert_eq!(
        instance["metadata"],
        json!({"width": 1.23456, "nested": {"x": 2.34567}})
    );
    assert_eq!(
        json["pages"][0]["children"][1]["inputs"]["scale"],
        json!(0.123456789)
    );
    assert_eq!(
        json["pages"][0]["children"][2]["fills"][0]["uniforms"],
        json!({"k": 0.123456789, "v": [0.12345, 0.98765]})
    );
}

#[test]
fn an_override_that_sets_properties_is_rounded_like_the_node_it_changes() {
    let doc = document(json!({
        "meta": meta(),
        "components": {"comp000001": {
            "name": "Card",
            "root": {"type": "frame", "id": "card00001", "name": "Card", "width": 100.0, "height": 50.0,
                     "children": [{"type": "rect", "id": "bg000001", "name": "Back", "width": 10.0, "height": 10.0}]},
            "variants": [{"properties": {"State": "hover"}, "overrides": {
                "card00001/bg000001": {"props": {"width": 12.3456, "opacity": 0.12349,
                    "fills": [{"type": "gradient", "kind": "linear",
                        "stops": [{"color": "#000000", "position": 0.12345}]}]}}
            }}]
        }},
        "pages": [{"id": "page000001", "name": "Canvas", "children": [
            {"type": "instance", "id": "i0000001", "name": "Card", "width": "hug", "height": "hug",
             "componentId": "comp000001",
             "overrides": {"card00001/bg000001": {"props": {"height": 7.777, "props": {"width": 1.23456}}}}}
        ]}]
    }));
    let json = printed(&doc);
    let patch = &json["components"]["comp000001"]["variants"][0]["overrides"]["card00001/bg000001"]
        ["props"];
    assert_eq!(patch["width"], json!(12.35));
    assert_eq!(patch["opacity"], json!(0.123));
    assert_eq!(patch["fills"][0]["stops"][0]["position"], json!(0.123));
    let instance_patch =
        &json["pages"][0]["children"][0]["overrides"]["card00001/bg000001"]["props"];
    assert_eq!(instance_patch["height"], json!(7.78));
    assert_eq!(
        instance_patch["props"]["width"],
        json!(1.23456),
        "a prop of a nested instance is a prop"
    );
}

#[test]
fn a_node_outside_the_page_tree_is_rounded_too() {
    let doc = document(json!({
        "meta": meta(),
        "components": {"comp000001": {
            "name": "Card",
            "root": {"type": "frame", "id": "card00001", "name": "Card", "width": 100.126, "height": 50.0,
                     "children": [{"type": "rect", "id": "bg000001", "name": "Back", "width": 10.004, "height": 10.0}]}
        }},
        "pages": [{"id": "page000001", "name": "Canvas", "children": [
            {"type": "instance", "id": "i0000001", "name": "Card", "width": "hug", "height": "hug",
             "componentId": "comp000001",
             "overrides": {"card00001/bg000001": {"replace":
                {"type": "rect", "id": "bg000001", "name": "Back", "width": 11.116, "height": 10.0}}},
             "slotContent": {"slot00001": [{"type": "rect", "id": "s2000001", "name": "S", "width": 5.556, "height": 1.0}]}}
        ]}]
    }));
    let json = printed(&doc);
    assert_eq!(
        json["components"]["comp000001"]["root"]["children"][0]["width"],
        json!(10)
    );
    let instance = &json["pages"][0]["children"][0];
    assert_eq!(
        instance["overrides"]["card00001/bg000001"]["replace"]["width"],
        json!(11.12)
    );
    assert_eq!(
        instance["slotContent"]["slot00001"][0]["width"],
        json!(5.56)
    );
}

// --- quantization -------------------------------------------------------------------------------

#[test]
fn each_quantum_has_its_step() {
    assert_eq!(quantize(1.236, Quantum::Pixels), 1.24);
    assert_eq!(quantize(1.2349, Quantum::Ratio), 1.235);
    assert_eq!(quantize(1.236, Quantum::Degrees), 1.24);
    assert_eq!(
        quantize(-1.236, Quantum::Pixels),
        -1.24,
        "halves round away from zero"
    );
    assert_eq!(quantize(0.125, Quantum::Pixels), 0.13);
    assert_eq!(quantize(-0.004, Quantum::Pixels), 0.0);
    assert!(
        quantize(-0.004, Quantum::Pixels).is_sign_positive(),
        "no negative zero"
    );
}

#[test]
fn quantizing_twice_is_quantizing_once_at_every_magnitude() {
    for quantum in [Quantum::Pixels, Quantum::Ratio, Quantum::Degrees] {
        for value in [
            0.0,
            0.005,
            0.0049999,
            1.0 / 3.0,
            2.0 / 3.0,
            123.456,
            -987.654321,
            1.0e6 + 0.123,
            4.0e12 + 0.5,
            1.0e15,
            1.0e300,
            f64::MAX,
            f64::MIN_POSITIVE,
            5.0e-324,
            -5.0e-324,
        ] {
            let once = quantize(value, quantum);
            assert_eq!(quantize(once, quantum), once, "{value} as {quantum:?}");
            assert!(once.is_finite());
        }
    }
}

#[test]
fn a_value_too_large_to_have_a_fraction_is_left_as_it_is() {
    assert_eq!(quantize(1.0e300, Quantum::Pixels), 1.0e300);
    assert_eq!(quantize(f64::MAX, Quantum::Ratio), f64::MAX);
}

#[test]
fn live_state_is_never_rounded_only_the_bytes_are() {
    let doc = with_node(json!({"type": "rect", "id": "r0000001", "name": "Back",
        "width": 100.126, "height": 10.0}));
    let json = serde_json::to_value(&doc).expect("serialise");
    assert_eq!(json["pages"][0]["children"][0]["width"], json!(100.126));
    let back = decode(&encode(&doc).expect("encode")).expect("decode");
    assert_eq!(
        serde_json::to_value(&back).expect("serialise")["pages"][0]["children"][0]["width"],
        json!(100.13)
    );
}

// --- refusing what cannot be written ------------------------------------------------------------

#[test]
fn a_number_that_is_not_finite_is_refused_with_the_place_it_sits() {
    let mut doc = with_node(json!({"type": "rect", "id": "r0000001", "name": "Back",
        "width": 10.0, "height": 10.0, "opacity": 0.5}));
    if let vellum_types::Node::Rect(rect) = &mut doc.pages[0].children[0] {
        rect.opacity = vellum_types::Value::Literal(f64::NAN);
    }
    match encode(&doc) {
        Err(CodecError::NotFinite { path }) => assert_eq!(path, "pages[0].children[0].opacity"),
        other => panic!("{other:?}"),
    }
    let message = encode(&doc).expect_err("refused").to_string();
    assert!(
        message.contains("pages[0].children[0].opacity"),
        "{message}"
    );
}

#[test]
fn infinity_is_refused_wherever_it_sits() {
    let mut doc = with_node(json!({"type": "rect", "id": "r0000001", "name": "Back",
        "width": 10.0, "height": 10.0, "metadata": {"k": 1}}));
    if let vellum_types::Node::Rect(rect) = &mut doc.pages[0].children[0] {
        rect.metadata.insert("k".to_owned(), json!(1.5));
        rect.height = vellum_types::Size::Fixed(vellum_types::Value::Literal(f64::INFINITY));
    }
    assert!(matches!(encode(&doc), Err(CodecError::NotFinite { .. })));
}

#[test]
fn a_document_whose_version_is_not_this_build_is_not_written() {
    let mut doc = Document::new("TEST DATA");
    doc.meta.format_version = 2;
    match encode(&doc) {
        Err(CodecError::WrongVersion { found, expected }) => {
            assert_eq!((found, expected), (2, vellum_types::FORMAT_VERSION));
        }
        other => panic!("{other:?}"),
    }
}

// --- the codec as a port ------------------------------------------------------------------------

#[test]
fn the_codec_names_itself_and_writes_what_the_free_functions_write() {
    let codec = JsonCodec::new();
    assert_eq!(codec.id(), "json");
    let doc = Document::new("TEST DATA");
    assert_eq!(
        codec.encode(&doc).expect("encode"),
        encode(&doc).expect("encode")
    );
    assert_eq!(
        codec
            .decode(&encode(&doc).expect("encode"))
            .expect("decode"),
        doc
    );
}
