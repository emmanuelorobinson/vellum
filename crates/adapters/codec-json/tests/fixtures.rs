//! Every fixture under `fixtures/documents/` is a canonical `.vellum` file: loading it and saving it gives the
//! same bytes (Invariant 5). Together they use every kind of node, paint, effect, variable and style.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value as Json;
use vellum_codec_json::{decode, encode};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/documents")
}

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let mut found: Vec<_> = std::fs::read_dir(dir())
        .expect("fixtures/documents exists")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "vellum"))
        .map(|path| {
            let name = path
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            (name, std::fs::read(&path).expect("readable"))
        })
        .collect();
    found.sort();
    found
}

fn collect(value: &Json, key: &str, out: &mut BTreeSet<String>) {
    match value {
        Json::Object(map) => {
            for (k, v) in map {
                if k == key
                    && let Some(text) = v.as_str()
                {
                    out.insert(text.to_owned());
                }
                collect(v, key, out);
            }
        }
        Json::Array(items) => items.iter().for_each(|v| collect(v, key, out)),
        _ => {}
    }
}

#[test]
fn there_are_fixtures() {
    assert!(fixtures().len() >= 5, "found {}", fixtures().len());
}

#[test]
fn loading_a_fixture_and_saving_it_gives_the_same_bytes() {
    for (name, bytes) in fixtures() {
        let doc = decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let saved = encode(&doc).expect("encode");
        assert_eq!(
            String::from_utf8_lossy(&saved),
            String::from_utf8_lossy(&bytes),
            "{name} is not in canonical form"
        );
    }
}

#[test]
fn every_fixture_is_marked_as_test_data() {
    for (name, bytes) in fixtures() {
        let doc = decode(&bytes).expect("decode");
        assert!(
            doc.meta.name.starts_with("TEST DATA"),
            "{name}: {}",
            doc.meta.name
        );
    }
}

#[test]
fn a_fixture_with_crlf_line_endings_loads_to_the_same_bytes() {
    for (name, bytes) in fixtures() {
        let text = String::from_utf8(bytes.clone())
            .expect("utf-8")
            .replace('\n', "\r\n");
        let doc = decode(text.as_bytes()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(encode(&doc).expect("encode"), bytes, "{name}");
    }
}

#[test]
fn the_fixtures_between_them_use_every_kind_of_thing_the_format_has() {
    let mut types = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut page_kinds = BTreeSet::new();
    let mut text = String::new();
    for (_, bytes) in fixtures() {
        let json: Json = serde_json::from_slice(&bytes).expect("json");
        collect(&json, "type", &mut types);
        collect(&json, "kind", &mut kinds);
        collect(&json["pages"], "kind", &mut page_kinds);
        text.push_str(&String::from_utf8(bytes).expect("utf-8"));
    }
    for wanted in [
        "frame",
        "group",
        "rect",
        "ellipse",
        "polygon",
        "star",
        "line",
        "vector",
        "boolean",
        "text",
        "image",
        "icon",
        "instance",
        "script",
        "note",
        "context",
        "prompt",
        "embed", // nodes
        "solid",
        "gradient",
        "mesh",
        "shader", // paints
        "drop-shadow",
        "inner-shadow",
        "layer-blur",
        "background-blur", // effects
        "color",
        "number",
        "string",
        "boolean", // variables
        "effect",  // styles ("text" is a node type too)
        "absolute",
    ] {
        assert!(types.contains(wanted), "no fixture uses type {wanted}");
    }
    assert!(kinds.contains("library"), "no fixture is a library");
    assert!(
        page_kinds.contains("slides"),
        "no fixture has a slides page"
    );
    for wanted in [
        "\"$var\"",
        "\"$alias\"",
        "\"imports\"",
        "\"themes\"",
        "\"fonts\"",
        "\"styles\"",
        "\"components\"",
        "\"variants\"",
        "\"overrides\"",
        "\"props\":",
        "\"replace\"",
        "\"children\":",
        "\"slotContent\"",
        "\"when\"",
        "\"metadata\"",
        "\"prototype\"",
        "\"hug\"",
        "\"fill\"",
    ] {
        assert!(text.contains(wanted), "no fixture uses {wanted}");
    }
}
