//! The two on-disk formats with the real codec (guide Prompt 24): switching between them is lossless over every
//! fixture, errors keep their file and position, and the real disk behaves like the fake one.
//! The composition root is where adapters meet, so the adapter-to-adapter checks live here.

use std::path::{Path, PathBuf};

use vellum_assets_fs::StdFs;
use vellum_codec_json::JsonCodec;
use vellum_codec_json_dir::{DirCodec, INDEX, PAGES};
use vellum_contracts::fakes::MemFs;
use vellum_core::files::SingleFile;
use vellum_core::ports::{CodecError, DocumentFiles, FileError, FileSystem};
use vellum_types::Document;

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut found = Vec::new();
    for folder in ["documents", "resolver"] {
        for entry in std::fs::read_dir(root.join(folder)).expect("fixtures folder") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "vellum") {
                let name = format!(
                    "{folder}/{}",
                    path.file_name().and_then(|n| n.to_str()).unwrap()
                );
                found.push((name, std::fs::read(&path).expect("read fixture")));
            }
        }
    }
    found.sort();
    assert!(found.len() >= 7, "fixtures went missing: {found:?}");
    found
}

#[test]
fn switching_from_one_file_to_a_folder_and_back_gives_the_same_bytes() {
    let json = JsonCodec::new();
    let (single, dir) = (SingleFile::new(&json), DirCodec::new(&json));
    for (name, bytes) in fixtures() {
        let doc = json
            .decode(&bytes)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let fs = MemFs::new();
        dir.write(&fs, "app.vellum", &doc).expect(&name);
        let loaded = dir.read(&fs, "app.vellum").expect(&name);
        assert_eq!(
            loaded.document, doc,
            "{name}: a folder changed the document"
        );
        assert!(loaded.orphans.is_empty() && !loaded.interrupted, "{name}");
        assert_eq!(
            json.encode(&loaded.document).unwrap(),
            bytes,
            "{name}: single file to folder to single file changed the bytes"
        );

        // And the other way: a folder read, written as one file, read as a folder again.
        single
            .write(&fs, "one.vellum", &loaded.document)
            .expect(&name);
        let back = single.read(&fs, "one.vellum").expect(&name).document;
        dir.write(&fs, "again.vellum", &back).expect(&name);
        for file in files_in(&fs, "app.vellum") {
            assert_eq!(
                fs.read(&format!("app.vellum/{file}")).unwrap(),
                fs.read(&format!("again.vellum/{file}")).unwrap(),
                "{name}: {file} differs after a trip through a single file"
            );
        }
    }
}

fn files_in(fs: &MemFs, dir: &str) -> Vec<String> {
    let mut files = vec![INDEX.to_owned()];
    files.extend(
        fs.list(&format!("{dir}/{PAGES}"))
            .unwrap()
            .into_iter()
            .map(|n| format!("{PAGES}/{n}")),
    );
    files
}

#[test]
fn the_real_codec_reports_a_bad_page_with_its_file_line_and_column() {
    let json = JsonCodec::new();
    let dir = DirCodec::new(&json);
    let (name, bytes) = fixtures()
        .into_iter()
        .find(|(n, _)| n.ends_with("shapes.vellum"))
        .unwrap();
    let doc = json.decode(&bytes).unwrap();
    let fs = MemFs::new();
    dir.write(&fs, "app.vellum", &doc).unwrap();

    let page = &doc.pages[0];
    let file = format!("app.vellum/{PAGES}/{}.json", page.id);
    // An unknown field: DESIGN S05 wants the path, the line and the column.
    let text = String::from_utf8(fs.read(&file).unwrap()).unwrap();
    let broken = text.replacen("\"pages\": [", "\"pages\": [\n    {\"bogus\": 1},", 1);
    assert_ne!(broken, text, "{name}: the page file has a different shape");
    fs.seed(&file, broken.as_bytes());
    match dir.read(&fs, "app.vellum") {
        Err(FileError::Codec(CodecError::Invalid {
            message, position, ..
        })) => {
            assert!(message.contains(&format!("{}.json", page.id)), "{message}");
            assert!(position.is_some_and(|p| p.line >= 1), "{position:?}");
        }
        other => panic!("{other:?}"),
    }

    // Not JSON at all.
    fs.seed(&file, b"{ \"meta\": ");
    match dir.read(&fs, "app.vellum") {
        Err(FileError::Codec(CodecError::Syntax { position, message })) => {
            assert_eq!(position.line, 1);
            assert!(message.contains("in pages/"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

/// A folder under the system temp dir, removed afterwards.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("vellum-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn both_formats_work_on_a_real_disk() {
    let json = JsonCodec::new();
    let tmp = TempDir::new("formats");
    let fs = StdFs::new(&tmp.0);
    for (name, bytes) in fixtures() {
        let doc = json.decode(&bytes).unwrap();
        let slug = name.replace('/', "-");

        SingleFile::new(&json)
            .write(&fs, &format!("{slug}.single.vellum"), &doc)
            .unwrap();
        assert_eq!(
            std::fs::read(tmp.0.join(format!("{slug}.single.vellum"))).unwrap(),
            bytes,
            "{name}: the single file is the fixture, byte for byte"
        );

        let folder = format!("{slug}.dir.vellum");
        let dir = DirCodec::new(&json);
        dir.write(&fs, &folder, &doc).unwrap();
        assert!(tmp.0.join(&folder).join(INDEX).is_file());
        assert!(!tmp.0.join(&folder).join(".saving").exists());
        assert_eq!(dir.read(&fs, &folder).unwrap().document, doc, "{name}");
        // Nothing but our files: no temporary file was left on the real disk.
        for entry in std::fs::read_dir(tmp.0.join(&folder).join(PAGES)).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                name.ends_with(".json") && !name.starts_with('.'),
                "stray file {name}"
            );
        }
    }
}

/// ADR-004 sets the multi-file suggestion at 10,000 nodes or 4 MB and lets this prompt move the numbers with the
/// real codec. Ignored by default (about 10 s in a debug build); run it in release:
/// `cargo test -p vellum --release --test files -- --ignored --nocapture measure`.
#[test]
#[ignore = "measurement for ADR-018; run with --release --ignored --nocapture"]
fn measure_the_large_file_threshold_with_the_real_codec() {
    use std::time::Instant;
    use vellum_core::files::{count_nodes, suggest_multi_file};

    let json = JsonCodec::new();
    println!(
        "nodes,json_bytes,suggested,encode_ms,decode_ms,dir_full_save_ms,dir_one_page_save_ms,dir_one_page_bytes,dir_read_ms,disk_single_save_ms,disk_dir_full_save_ms,disk_dir_one_page_save_ms"
    );
    let disk = TempDir::new("measure");
    let real = StdFs::new(&disk.0);
    for frames in [25, 50, 100, 200, 400] {
        // One page of `frames` frames with 99 rectangles each, as ADR-004 measured.
        let mut doc = Document::new("TEST DATA: large");
        let mut children = Vec::new();
        for f in 0..frames {
            let rects: Vec<_> = (0..99)
                .map(|r| {
                    vellum_contracts::samples::node_with(
                        "rect",
                        &format!("r{f:04}x{r:03}"),
                        &[],
                        serde_json::json!({
                            "position": {"type": "absolute", "x": r as f64 * 10.0, "y": f as f64 * 4.0},
                            "fills": [{"type": "solid", "color": "#336699"}]
                        }),
                    )
                })
                .collect();
            children.push(vellum_contracts::samples::node_with(
                "frame",
                &format!("f{f:04}"),
                &rects,
                serde_json::json!({}),
            ));
        }
        // Ten pages, as a real file would have, so one page edit is a tenth of the document.
        let per_page = children.len().div_ceil(10);
        for (n, chunk) in children.chunks(per_page).enumerate() {
            doc.pages.push(vellum_contracts::samples::page(
                &format!("page{n:05}"),
                chunk.to_vec(),
            ));
        }

        let nodes = count_nodes(&doc);
        let t = Instant::now();
        let bytes = json.encode(&doc).unwrap();
        let encode = t.elapsed();
        let t = Instant::now();
        let back = json.decode(&bytes).unwrap();
        let decode = t.elapsed();
        assert_eq!(back.pages.len(), doc.pages.len());

        let fs = MemFs::new();
        let dir = DirCodec::new(&json);
        let t = Instant::now();
        dir.write(&fs, "big.vellum", &doc).unwrap();
        let full = t.elapsed();
        let mut edited = doc.clone();
        edited.pages[3].children.pop();
        let t = Instant::now();
        let saved = dir.write(&fs, "big.vellum", &edited).unwrap();
        let one = t.elapsed();
        let page_bytes: usize = saved
            .written
            .iter()
            .map(|f| fs.read(&format!("big.vellum/{f}")).unwrap().len())
            .sum();
        let t = Instant::now();
        dir.read(&fs, "big.vellum").unwrap();
        let read = t.elapsed();
        // The same saves on the real disk, where every file costs an fsync.
        let single = SingleFile::new(&json);
        single.write(&real, "single.vellum", &doc).unwrap();
        let t = Instant::now();
        single.write(&real, "single.vellum", &edited).unwrap();
        let disk_single = t.elapsed();
        let folder = format!("big-{frames}.vellum");
        let t = Instant::now();
        dir.write(&real, &folder, &doc).unwrap();
        let disk_full = t.elapsed();
        let mut edited_again = edited.clone();
        edited_again.pages[3].children.pop();
        let t = Instant::now();
        dir.write(&real, &folder, &edited).unwrap();
        let disk_one = t.elapsed();
        let _ = edited_again;
        println!(
            "{nodes},{},{},{:.1},{:.1},{:.1},{:.1},{page_bytes},{:.1},{:.1},{:.1},{:.1}",
            bytes.len(),
            suggest_multi_file(nodes, bytes.len()),
            encode.as_secs_f64() * 1e3,
            decode.as_secs_f64() * 1e3,
            full.as_secs_f64() * 1e3,
            one.as_secs_f64() * 1e3,
            read.as_secs_f64() * 1e3,
            disk_single.as_secs_f64() * 1e3,
            disk_full.as_secs_f64() * 1e3,
            disk_one.as_secs_f64() * 1e3,
        );
    }
}
