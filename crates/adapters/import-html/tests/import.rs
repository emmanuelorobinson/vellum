//! TEST DATA. Computed browser geometry becomes editable layers, without another write path.
use serde_json::{Value, json};
use vellum_core::ports::{ImportInput, Importer};
use vellum_import_html::HtmlImporter;

fn snapshot() -> Value {
    json!({"version":1,"width":320,"height":240,"nodes":[
        {"kind":"frame","name":"article","x":0,"y":0,"width":320,"height":240,
         "style":{"backgroundColor":"rgb(255, 255, 255)","borderRadius":"8px","opacity":"1"}},
        {"kind":"text","name":"text","parent":0,"x":16,"y":20,"width":200,"height":24,
         "text":"Editable text","style":{"color":"rgb(20, 30, 40)","fontFamily":"Inter, sans-serif","fontSize":"20px","fontWeight":"600","lineHeight":"24px"}},
        {"kind":"image","name":"img","parent":0,"x":16,"y":60,"width":32,"height":32,
         "image":"data:image/png;base64,iVBORw0KGgo=","style":{"objectFit":"contain"}}
    ],"notes":[]})
}
fn run(value: &Value) -> vellum_core::ports::Imported {
    HtmlImporter
        .import(&ImportInput {
            bytes: &serde_json::to_vec(value).unwrap(),
            name: "Page",
        })
        .unwrap()
}
#[test]
fn geometry_text_and_assets_are_editable_and_relative() {
    let result = run(&snapshot());
    let root = &result.nodes[0];
    assert_eq!(root["type"], "frame");
    assert_eq!(root["width"], 320.0);
    assert_eq!(root["cornerRadius"], 8.0);
    assert_eq!(root["children"][0]["type"], "text");
    assert_eq!(root["children"][0]["content"][0]["text"], "Editable text");
    assert_eq!(root["children"][0]["position"]["x"], 16.0);
    assert_eq!(root["children"][0]["content"][0]["size"], 20.0);
    assert_eq!(root["children"][1]["asset"], "import:0");
    assert_eq!(result.assets[0].mime, "image/png");
    assert_eq!(result.assets[0].bytes, b"\x89PNG\r\n\x1a\n");
}
#[test]
fn contract_and_determinism() {
    vellum_contracts::suites::import::importer(
        &HtmlImporter,
        &serde_json::to_vec(&snapshot()).unwrap(),
    );
    assert_eq!(run(&snapshot()), run(&snapshot()));
}
#[test]
fn malformed_foreign_and_oversized_snapshots_are_errors() {
    for value in [
        json!({}),
        json!({"version":2}),
        json!("<html/>"),
        {
            let mut s = snapshot();
            s["nodes"][0]["width"] = json!(-1);
            s
        },
        {
            let mut s = snapshot();
            s["nodes"][0]["parent"] = json!(1);
            s
        },
    ] {
        assert!(
            HtmlImporter
                .import(&ImportInput {
                    bytes: &serde_json::to_vec(&value).unwrap(),
                    name: ""
                })
                .is_err()
        );
    }
}
#[test]
fn unsupported_styles_and_missing_images_are_reported() {
    let mut s = snapshot();
    s["nodes"][0]["style"]["backgroundImage"] = json!("linear-gradient(red, blue)");
    s["nodes"][2]["image"] = Value::Null;
    let r = run(&s);
    assert!(r.notes.iter().any(|n| n.contains("background")));
    assert!(r.notes.iter().any(|n| n.contains("image")));
    assert_eq!(r.nodes[0]["children"].as_array().unwrap().len(), 1);
}
