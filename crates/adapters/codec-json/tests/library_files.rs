//! Libraries on disk (guide Prompt 24, ARCHITECTURE §4.4): turning a document into one, reading `*.lib.vellum`
//! files through the real codec, last-known copies in the cache folder, and carrying asset bytes on paste.

use std::sync::Arc;

use vellum_codec_json::JsonCodec;
use vellum_contracts::fakes::{InMemoryAssets, MemFs, SequentialIds};
use vellum_core::files::{
    FileLibrarySource, LibraryCache, LibraryError, copy_dependencies, library_path_for,
    turn_into_library,
};
use vellum_core::ops::{Op, Shell, Transaction, apply_edits, plan};
use vellum_core::ports::{AssetStore, FileSystem};
use vellum_core::resolver::libraries::Libraries;
use vellum_types::{Actor, DeviceId, Document, DocumentKind, Origin};

fn origin() -> Origin {
    Origin::new(Actor::Person {
        device: DeviceId::new("01JA0000000000000000000001").expect("a ULID"),
    })
}

fn library(name: &str, themes: &[&str]) -> Document {
    let mut doc = Document::new(format!("TEST DATA: {name}"));
    doc.meta.kind = DocumentKind::Library;
    doc.themes.insert(
        "mode".into(),
        themes.iter().map(|t| (*t).to_owned()).collect(),
    );
    doc
}

fn put(fs: &MemFs, path: &str, doc: &Document) {
    fs.seed(path, &JsonCodec::new().encode(doc).expect("encode"));
}

// --- Turn into library ---

#[test]
fn a_document_becomes_a_library_through_one_shell_op_and_undo_still_works() {
    let doc = Document::new("TEST DATA: design system");
    let shell = turn_into_library(&doc).expect("a document can become a library");
    assert_eq!(shell.meta.kind, DocumentKind::Library);
    assert_eq!(
        Shell {
            meta: doc.meta.clone(),
            ..shell.clone()
        },
        Shell::of(&doc),
        "nothing but the kind changes"
    );

    // The only write path is an op transaction; undo is the inverse edits, which are not the "turn back"
    // command (there is none).
    let mut work = doc.clone();
    let tx = Transaction::new(
        origin(),
        vec![Op::SetShell {
            shell: Box::new(shell),
        }],
    );
    let plan = plan(&work, &tx, &mut SequentialIds::new()).expect("planned");
    apply_edits(&mut work, &plan.edits, origin()).expect("applied");
    assert_eq!(work.meta.kind, DocumentKind::Library);
    apply_edits(&mut work, &plan.inverse, origin()).expect("undone");
    assert_eq!(work, doc);
}

#[test]
fn a_library_cannot_be_turned_into_a_library_again_and_there_is_no_way_back() {
    let lib = library("already", &["light"]);
    assert_eq!(turn_into_library(&lib), Err(LibraryError::AlreadyLibrary));
}

#[test]
fn the_library_file_name_is_the_document_name_with_lib_before_vellum() {
    assert_eq!(
        library_path_for("designs/app.vellum").unwrap(),
        "designs/app.lib.vellum"
    );
    assert_eq!(library_path_for("app.vellum").unwrap(), "app.lib.vellum");
    assert_eq!(
        library_path_for("a/brand.lib.vellum").unwrap(),
        "a/brand.lib.vellum"
    );
    assert!(matches!(
        library_path_for("notes.txt"),
        Err(LibraryError::NotAVellumFile(_))
    ));
}

// --- Reading from disk ---

#[test]
fn a_library_is_read_from_disk_missing_and_unreadable_are_different() {
    let fs = MemFs::new();
    put(&fs, "proj/ui.lib.vellum", &library("ui", &["light"]));
    fs.seed("proj/broken.lib.vellum", b"{ not json");
    let codec = JsonCodec::new();
    let source = FileLibrarySource::new(&fs, &codec);
    use vellum_core::resolver::libraries::LibrarySource;

    let doc = source.load("proj/ui.lib.vellum").unwrap().expect("there");
    assert_eq!(doc.themes["mode"], ["light"]);
    assert_eq!(source.load("proj/gone.lib.vellum").unwrap(), None);
    let why = source.load("proj/broken.lib.vellum").unwrap_err();
    assert!(why.contains("not valid JSON"), "S05 words: {why}");
    assert!(
        source.load("/proj/ui.lib.vellum").unwrap().is_some(),
        "an absolute path is read from the file system's root"
    );
    assert!(source.load("../outside.lib.vellum").is_err());
}

// --- The cache ---

#[test]
fn a_cached_copy_keeps_a_deleted_library_rendering() {
    let project = MemFs::new();
    let cache_fs = MemFs::new();
    let codec = JsonCodec::new();
    put(
        &project,
        "proj/ui.lib.vellum",
        &library("ui", &["light", "dark"]),
    );

    // A resolve pass reads the library and the host saves what it learned.
    {
        let source = FileLibrarySource::new(&project, &codec);
        let mut libraries = Libraries::new(&source);
        libraries.begin();
        let found = libraries.get("proj/app.vellum", "ui.lib.vellum").unwrap();
        assert!(!found.missing);
        let cache = LibraryCache::new(&cache_fs, &codec, "libraries");
        let report = cache.persist(&libraries).expect("persist");
        assert_eq!((report.written, report.unchanged), (1, 0));
        let again = cache.persist(&libraries).expect("persist again");
        assert_eq!(
            (again.written, again.unchanged),
            (0, 1),
            "an unchanged copy is not rewritten"
        );
    }

    // Next session the library is gone. The cache seeds the last known copy.
    project.remove_file("proj/ui.lib.vellum").unwrap();
    let source = FileLibrarySource::new(&project, &codec);
    let mut libraries = Libraries::new(&source);
    let cache = LibraryCache::new(&cache_fs, &codec, "libraries");
    let warmed = cache.warm(&mut libraries, &["proj/ui.lib.vellum"]);
    assert_eq!(warmed.seeded, vec!["proj/ui.lib.vellum".to_owned()]);
    libraries.begin();
    let found = libraries.get("proj/app.vellum", "ui.lib.vellum").unwrap();
    assert!(
        found.missing,
        "the file is gone, and the editor must say so"
    );
    let doc = found.document.expect("the last known copy");
    assert_eq!(doc.themes["mode"], ["light", "dark"]);
}

#[test]
fn a_library_that_is_back_replaces_its_cached_copy() {
    let project = MemFs::new();
    let cache_fs = MemFs::new();
    let codec = JsonCodec::new();
    let path = "proj/ui.lib.vellum";
    for themes in [&["one"][..], &["two"][..]] {
        put(&project, path, &library("ui", themes));
        let source = FileLibrarySource::new(&project, &codec);
        let mut libraries = Libraries::new(&source);
        let cache = LibraryCache::new(&cache_fs, &codec, "libraries");
        cache.warm(&mut libraries, &[path]);
        libraries.begin();
        libraries.get("proj/app.vellum", "ui.lib.vellum").unwrap();
        cache.persist(&libraries).unwrap();
    }
    project.remove_file(path).unwrap();
    let source = FileLibrarySource::new(&project, &codec);
    let mut libraries = Libraries::new(&source);
    LibraryCache::new(&cache_fs, &codec, "libraries").warm(&mut libraries, &[path]);
    libraries.begin();
    let found = libraries.get("proj/app.vellum", "ui.lib.vellum").unwrap();
    assert_eq!(found.document.unwrap().themes["mode"], ["two"]);
}

#[test]
fn two_libraries_with_the_same_name_in_different_folders_do_not_share_a_copy() {
    let project = MemFs::new();
    let cache_fs = MemFs::new();
    let codec = JsonCodec::new();
    put(&project, "a/ui.lib.vellum", &library("a", &["from-a"]));
    put(&project, "b/ui.lib.vellum", &library("b", &["from-b"]));
    let source = FileLibrarySource::new(&project, &codec);
    let mut libraries = Libraries::new(&source);
    libraries.begin();
    libraries.get("a/app.vellum", "ui.lib.vellum").unwrap();
    libraries.get("b/app.vellum", "ui.lib.vellum").unwrap();
    let cache = LibraryCache::new(&cache_fs, &codec, "libraries");
    assert_eq!(cache.persist(&libraries).unwrap().written, 2);

    let nothing = MemFs::new();
    let source = FileLibrarySource::new(&nothing, &codec);
    let mut fresh = Libraries::new(&source);
    cache.warm(&mut fresh, &["a/ui.lib.vellum", "b/ui.lib.vellum"]);
    fresh.begin();
    let a = fresh.get("a/app.vellum", "ui.lib.vellum").unwrap();
    let b = fresh.get("b/app.vellum", "ui.lib.vellum").unwrap();
    assert_eq!(a.document.unwrap().themes["mode"], ["from-a"]);
    assert_eq!(b.document.unwrap().themes["mode"], ["from-b"]);
}

#[test]
fn a_cache_entry_that_is_not_ours_or_is_damaged_is_skipped_with_a_reason() {
    let project = MemFs::new();
    let cache_fs = MemFs::new();
    let codec = JsonCodec::new();
    put(&project, "proj/ui.lib.vellum", &library("ui", &["x"]));
    let source = FileLibrarySource::new(&project, &codec);
    let mut libraries = Libraries::new(&source);
    libraries.begin();
    libraries.get("proj/app.vellum", "ui.lib.vellum").unwrap();
    let cache = LibraryCache::new(&cache_fs, &codec, "libraries");
    cache.persist(&libraries).unwrap();

    let entry = cache_fs
        .list("libraries")
        .unwrap()
        .into_iter()
        .find(|n| n.ends_with(".lib.vellum"))
        .expect("a cache file");
    cache_fs
        .write(&format!("libraries/{entry}"), b"{ half a file")
        .unwrap();

    let nothing = MemFs::new();
    let source = FileLibrarySource::new(&nothing, &codec);
    let mut fresh = Libraries::new(&source);
    let warmed = cache.warm(
        &mut fresh,
        &["proj/ui.lib.vellum", "proj/never-cached.lib.vellum"],
    );
    assert!(warmed.seeded.is_empty());
    assert_eq!(
        warmed.skipped.len(),
        1,
        "a path with no copy is just absent: {warmed:?}"
    );
    assert_eq!(warmed.skipped[0].0, "proj/ui.lib.vellum");
    assert!(
        warmed.skipped[0].1.contains("not valid JSON"),
        "{}",
        warmed.skipped[0].1
    );
}

// --- Paste carries the bytes ---

#[test]
fn pasting_copies_the_bytes_the_plan_names_and_reports_the_rest() {
    let from = InMemoryAssets::new();
    let to = Arc::new(InMemoryAssets::new());
    let image = from.put(b"TEST DATA image", "image/png").unwrap();
    let font = from.put(b"TEST DATA font", "font/ttf").unwrap();
    let absent = vellum_core::ports::AssetHash::new("ab".repeat(16)).unwrap();

    let report = copy_dependencies(
        &from,
        to.as_ref(),
        &[
            image.to_string(),
            format!("font:{font}"),
            format!("font:{absent}"),
            format!("lib.assets/{image}"),
            "shaders/glow.sksl".to_owned(),
            "scripts/chart.js".to_owned(),
        ],
    );
    assert_eq!(report.copied, vec![image.clone(), font.clone()]);
    assert_eq!(report.missing, vec![absent.to_string()]);
    assert_eq!(
        report.files,
        vec![
            "scripts/chart.js".to_owned(),
            "shaders/glow.sksl".to_owned()
        ],
        "paths are the host's to copy; the plan cannot move files"
    );
    assert_eq!(to.get(&image).unwrap(), b"TEST DATA image");
    assert_eq!(to.get(&font).unwrap(), b"TEST DATA font");
}
