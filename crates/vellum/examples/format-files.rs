//! Prints, as JSON, what `docs/format/files.md` shows: the files the two codecs really write for one small
//! document, the asset file name, the large-file thresholds and the migration steps (guide Prompt 24).
//!
//! `scripts/gen-format.mjs` runs this; run it by hand with `cargo run -p vellum --example format-files`.
//! Everything is `TEST DATA` and nothing reads the clock, so the output is the same every time.

use std::sync::Arc;

use serde_json::{Value, json};
use vellum_assets_fs::{FsAssets, assets_dir_for};
use vellum_codec_json::{JsonCodec, Migrations};
use vellum_codec_json_dir::{DirCodec, INDEX, PAGES};
use vellum_contracts::fakes::MemFs;
use vellum_core::files::{LARGE_FILE_BYTES, LARGE_FILE_NODES, SingleFile};
use vellum_core::ports::{AssetStore, DocumentFiles, FileSystem};
use vellum_types::Document;

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("canonical files are UTF-8")
}

fn main() {
    // Stored the way a real document stores them, so the ids below are real content hashes.
    let fs = Arc::new(MemFs::new());
    let assets = FsAssets::new(fs.clone(), assets_dir_for("app.vellum")).expect("a valid folder");
    let image = assets
        .put(b"TEST DATA: an image", "image/png")
        .expect("store the image");
    let font = assets
        .put(b"TEST DATA: a font", "font/woff2")
        .expect("store the font");

    let doc: Document = serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA: example", "createdWith": "Vellum 0.0.0"},
        "fonts": {font.as_str(): {"family": "TEST DATA Sans", "weight": 700}},
        "pages": [
            {"id": "page00001", "name": "Home", "children": [
                {"type": "rect", "id": "rect000001", "name": "Card", "width": 320, "height": 200,
                 "fills": [{"type": "image", "asset": image.as_str()}]}
            ]},
            {"id": "page00002", "name": "Settings"}
        ]
    }))
    .expect("a valid example document");

    let json = JsonCodec::new();
    let single = SingleFile::new(&json);
    single.write(&*fs, "app.vellum", &doc).expect("single file");
    let single_text = text(fs.read("app.vellum").expect("read it"));

    let dir = DirCodec::new(&json);
    dir.write(&*fs, "app-folder.vellum", &doc).expect("folder");
    let mut directory = serde_json::Map::new();
    let mut names = vec![INDEX.to_owned()];
    names.extend(
        fs.list(&format!("app-folder.vellum/{PAGES}"))
            .expect("list pages")
            .into_iter()
            .map(|n| format!("{PAGES}/{n}")),
    );
    for name in names {
        let bytes = fs.read(&format!("app-folder.vellum/{name}")).expect("read");
        directory.insert(name, Value::String(text(bytes)));
    }

    let migrations = Migrations::standard();
    let steps: Vec<Value> = migrations
        .steps()
        .iter()
        .map(|s| json!({"from": s.source_version(), "to": s.source_version() + 1}))
        .collect();

    let info = json!({
        "formatVersion": migrations.current(),
        "migrations": steps,
        "thresholds": {"nodes": LARGE_FILE_NODES, "bytes": LARGE_FILE_BYTES},
        "single": single_text,
        "directory": directory,
        "assets": {"folder": assets_dir_for("app.vellum"), "file": image.as_str()},
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&info).expect("the info serialises")
    );
}
