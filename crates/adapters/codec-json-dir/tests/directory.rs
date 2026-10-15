//! The directory format: one file per page and an index (guide Prompt 24, ARCHITECTURE §4.3).
//! Uses the contracts' fake JSON codec: this adapter takes its single-file codec through the `Codec` port, and
//! the tests with the real one (lossless switching over every fixture) live in `crates/vellum/tests`.

use vellum_codec_json_dir::{DirCodec, INDEX, PAGES, SAVING};
use vellum_contracts::fakes::{FakeJsonCodec, MemFs};
use vellum_contracts::{samples, suites};
use vellum_core::ports::{Codec, CodecError, DocumentFiles, FileError, FileSystem, FsError};
use vellum_types::{Document, Page};

const DIR: &str = "app.vellum";

fn page_file(page: &Page) -> String {
    format!("{DIR}/{PAGES}/{}.json", page.id)
}

fn two_pages() -> Document {
    let doc = samples::document();
    assert!(doc.pages.len() >= 2, "the sample has two pages");
    doc
}

fn written(files: &DirCodec, fs: &MemFs, doc: &Document) {
    files.write(fs, DIR, doc).expect("write");
}

#[test]
fn it_passes_the_document_files_suite() {
    let codec = FakeJsonCodec;
    suites::document_files::run(
        &DirCodec::new(&codec),
        &MemFs::new(),
        &[two_pages(), Document::new("TEST DATA: empty")],
    );
}

#[test]
fn the_folder_holds_an_index_and_one_file_per_page() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);

    assert_eq!(
        fs.list(DIR).unwrap(),
        vec![INDEX.to_owned(), PAGES.to_owned()]
    );
    let mut expected: Vec<String> = doc.pages.iter().map(|p| format!("{}.json", p.id)).collect();
    expected.sort();
    assert_eq!(fs.list(&format!("{DIR}/{PAGES}")).unwrap(), expected);

    // The index is a document with every page empty, in order: the shell and the page list.
    let index = codec
        .decode(&fs.read(&format!("{DIR}/{INDEX}")).unwrap())
        .unwrap();
    assert_eq!(index.meta, doc.meta);
    assert_eq!(index.themes, doc.themes);
    assert_eq!(index.variables, doc.variables);
    assert_eq!(index.components, doc.components);
    assert_eq!(
        index
            .pages
            .iter()
            .map(|p| (&p.id, &p.name, p.kind))
            .collect::<Vec<_>>(),
        doc.pages
            .iter()
            .map(|p| (&p.id, &p.name, p.kind))
            .collect::<Vec<_>>()
    );
    assert!(index.pages.iter().all(|p| p.children.is_empty()));

    // A page file is a document with one page and nothing else, so any tool that reads `.vellum` reads it.
    for page in &doc.pages {
        let one = codec.decode(&fs.read(&page_file(page)).unwrap()).unwrap();
        assert_eq!(one.pages, vec![page.clone()]);
        assert!(one.components.is_empty() && one.variables.is_empty() && one.fonts.is_empty());
    }
}

#[test]
fn the_same_document_makes_the_same_files() {
    let codec = FakeJsonCodec;
    let (files, doc) = (DirCodec::new(&codec), two_pages());
    let (a, b) = (MemFs::new(), MemFs::new());
    written(&files, &a, &doc);
    written(&files, &b, &doc);
    for name in [
        INDEX.to_owned(),
        format!("{PAGES}/{}.json", doc.pages[0].id),
    ] {
        assert_eq!(
            a.read(&format!("{DIR}/{name}")).unwrap(),
            b.read(&format!("{DIR}/{name}")).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn only_the_page_that_changed_is_rewritten() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);

    let again = fs.mutations();
    let saved = files.write(&fs, DIR, &doc).unwrap();
    assert_eq!(
        fs.mutations(),
        again,
        "saving what is already saved touched the disk"
    );
    assert_eq!((saved.written.len(), saved.unchanged), (0, 3));

    let mut edited = doc.clone();
    edited.pages[1].name = "TEST DATA renamed page".into();
    let saved = files.write(&fs, DIR, &edited).unwrap();
    // The page's name lives in the index (its stub) and in its own file.
    let mut names = saved.written.clone();
    names.sort();
    let mut expect = vec![
        INDEX.to_owned(),
        format!("{PAGES}/{}.json", doc.pages[1].id),
    ];
    expect.sort();
    assert_eq!(names, expect);
    assert_eq!(saved.unchanged, 1, "the other page stayed as it was");

    let mut shell_only = edited.clone();
    shell_only.meta.name = "TEST DATA renamed document".into();
    let saved = files.write(&fs, DIR, &shell_only).unwrap();
    assert_eq!(
        saved.written,
        vec![INDEX.to_owned()],
        "renaming the document must not rewrite every page"
    );
}

#[test]
fn a_document_without_pages_still_has_the_pages_folder() {
    let codec = FakeJsonCodec;
    let (files, fs) = (DirCodec::new(&codec), MemFs::new());
    written(&files, &fs, &Document::new("TEST DATA: no pages"));
    assert_eq!(
        fs.list(DIR).unwrap(),
        vec![INDEX.to_owned(), PAGES.to_owned()],
        "the layout does not depend on how many pages there are"
    );
    assert_eq!(
        fs.list(&format!("{DIR}/{PAGES}")).unwrap(),
        Vec::<String>::new()
    );
}

/// One changed file is already one atomic write. The marker is for saves that change several files at once, so
/// the commonest save (edit something on one page) costs one write, not three.
#[test]
fn a_save_that_changes_one_file_is_one_atomic_write_and_no_marker() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    let mut edited = doc.clone();
    edited.pages[0].children.pop();
    let before = fs.mutations();
    let saved = files.write(&fs, DIR, &edited).unwrap();
    assert_eq!(
        saved.written,
        vec![format!("{PAGES}/{}.json", doc.pages[0].id)]
    );
    assert_eq!(
        fs.mutations() - before,
        5,
        "make the folder, write, sync, rename, sync the folder: nothing more"
    );
    assert!(!fs.exists(&format!("{DIR}/{SAVING}")).unwrap());
}

#[test]
fn reordering_pages_rewrites_only_the_index() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    let mut reordered = doc.clone();
    reordered.pages.reverse();
    let saved = files.write(&fs, DIR, &reordered).unwrap();
    assert_eq!(saved.written, vec![INDEX.to_owned()]);
    assert_eq!(files.read(&fs, DIR).unwrap().document, reordered);
}

#[test]
fn a_deleted_page_loses_its_file_after_the_index_stops_naming_it() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    let mut fewer = doc.clone();
    let gone = fewer.pages.remove(0);
    let saved = files.write(&fs, DIR, &fewer).unwrap();
    assert_eq!(saved.removed, vec![format!("{PAGES}/{}.json", gone.id)]);
    assert!(!fs.exists(&page_file(&gone)).unwrap());
    assert_eq!(files.read(&fs, DIR).unwrap().document, fewer);
}

#[test]
fn files_the_index_does_not_name_are_reported_not_read() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    fs.seed(&format!("{DIR}/{PAGES}/zzzzzzzzzz.json"), b"{}");
    fs.seed(&format!("{DIR}/{PAGES}/notes.txt"), b"mine");
    fs.seed(&format!("{DIR}/{PAGES}/.half.json.vellum-tmp"), b"half");
    let loaded = files.read(&fs, DIR).unwrap();
    assert_eq!(
        loaded.document, doc,
        "orphans change nothing about the document"
    );
    assert_eq!(
        loaded.orphans,
        vec![
            format!("{PAGES}/notes.txt"),
            format!("{PAGES}/zzzzzzzzzz.json")
        ],
        "temporary files are ours to sweep and are not reported"
    );

    let saved = files.write(&fs, DIR, &doc).unwrap();
    assert_eq!(
        saved.removed,
        vec![format!("{PAGES}/zzzzzzzzzz.json")],
        "a write clears page files nobody names, and leaves a note someone put there"
    );
    assert_eq!(
        files.read(&fs, DIR).unwrap().orphans,
        vec![format!("{PAGES}/notes.txt")]
    );
    assert!(
        !fs.exists(&format!("{DIR}/{PAGES}/.half.json.vellum-tmp"))
            .unwrap(),
        "an interrupted write's temporary file is swept"
    );
}

#[test]
fn a_page_file_that_is_gone_is_an_error_naming_it() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    fs.remove_file(&page_file(&doc.pages[1])).unwrap();
    match files.read(&fs, DIR) {
        Err(FileError::Codec(CodecError::Invalid { path, message, .. })) => {
            assert_eq!(path, "pages[1]");
            assert!(message.contains("missing"), "{message}");
            assert!(
                message.contains(&format!("{}.json", doc.pages[1].id)),
                "{message}"
            );
        }
        other => panic!("a half-open document was returned: {other:?}"),
    }
}

#[test]
fn a_folder_without_an_index_and_a_path_with_nothing_are_different_errors() {
    let codec = FakeJsonCodec;
    let (files, fs) = (DirCodec::new(&codec), MemFs::new());
    assert!(matches!(
        files.read(&fs, "nothing.vellum"),
        Err(FileError::Fs(FsError::NotFound(_)))
    ));
    fs.seed("other.vellum/readme.txt", b"hi");
    match files.read(&fs, "other.vellum") {
        Err(FileError::Codec(CodecError::Invalid { message, .. })) => {
            assert!(message.contains("index.json"), "{message}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_error_inside_a_page_file_names_the_file_and_keeps_its_line_and_column() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    let broken = b"{\n  \"meta\": ";
    fs.seed(&page_file(&doc.pages[0]), broken);
    let in_the_file = codec.decode(broken).unwrap_err().position().unwrap();
    match files.read(&fs, DIR) {
        Err(FileError::Codec(CodecError::Syntax { position, message })) => {
            assert!(
                message.contains(&format!("{}.json", doc.pages[0].id)),
                "{message}"
            );
            assert_eq!(
                position, in_the_file,
                "the line and column are the file's own, not the folder's"
            );
            assert_eq!(position.line, 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_page_file_holding_another_page_is_refused() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    let other = fs.read(&page_file(&doc.pages[1])).unwrap();
    fs.seed(&page_file(&doc.pages[0]), &other);
    assert!(
        matches!(
            files.read(&fs, DIR),
            Err(FileError::Codec(CodecError::Invalid { .. }))
        ),
        "swapped page files must not load as the wrong page"
    );
}

#[test]
fn page_ids_that_one_disk_would_merge_are_refused_before_anything_is_written() {
    let codec = FakeJsonCodec;
    let files = DirCodec::new(&codec);
    let mut doc = Document::new("TEST DATA: collision");
    doc.pages.push(samples::page("AbCdEfGhIj", vec![]));
    doc.pages.push(samples::page("abcdefghij", vec![]));
    let fs = MemFs::new();
    match files.write(&fs, DIR, &doc) {
        Err(FileError::Codec(CodecError::Invalid { message, .. })) => {
            assert!(message.contains("only in case"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fs.mutations(), 0);

    let mut reserved = Document::new("TEST DATA: reserved");
    reserved.pages.push(samples::page("nul", vec![]));
    assert!(matches!(
        files.write(&fs, DIR, &reserved),
        Err(FileError::Codec(CodecError::Invalid { .. }))
    ));
}

#[test]
fn paths_outside_the_root_are_refused() {
    let codec = FakeJsonCodec;
    let files = DirCodec::new(&codec);
    let fs = MemFs::new();
    assert!(matches!(
        files.write(&fs, "../escape.vellum", &two_pages()),
        Err(FileError::Fs(FsError::InvalidPath(_)))
    ));
}

#[test]
fn a_marker_says_a_save_was_interrupted_and_the_next_save_clears_it() {
    let codec = FakeJsonCodec;
    let (files, fs, doc) = (DirCodec::new(&codec), MemFs::new(), two_pages());
    written(&files, &fs, &doc);
    assert!(
        !fs.exists(&format!("{DIR}/{SAVING}")).unwrap(),
        "a finished save leaves no marker"
    );
    fs.seed(&format!("{DIR}/{SAVING}"), b"");
    assert!(files.read(&fs, DIR).unwrap().interrupted);

    let mut edited = doc.clone();
    edited.pages[0].name = "TEST DATA after".into();
    files.write(&fs, DIR, &edited).unwrap();
    assert!(!files.read(&fs, DIR).unwrap().interrupted);

    // Even a save that finds nothing to change finishes an interrupted one: the marker goes.
    fs.seed(&format!("{DIR}/{SAVING}"), b"");
    files.write(&fs, DIR, &edited).unwrap();
    assert!(!fs.exists(&format!("{DIR}/{SAVING}")).unwrap());
}

/// Pull the plug at every step of a save that rewrites a page, adds one and drops one. The disk afterwards
/// holds only whole files, reads back, and is the old document, the new one, or says it was interrupted.
#[test]
fn a_crash_at_any_step_leaves_whole_files_and_an_honest_read() {
    let codec = FakeJsonCodec;
    let files = DirCodec::new(&codec);
    let old = two_pages();
    let mut new = old.clone();
    new.pages.remove(0);
    new.pages[0].name = "TEST DATA changed".into();
    new.pages.push(samples::page(
        "pagenew001",
        vec![samples::node("rect", "newrect0001")],
    ));
    new.meta.name = "TEST DATA new name".into();

    let probe = MemFs::new();
    written(&files, &probe, &old);
    let before = probe.mutations();
    files.write(&probe, DIR, &new).unwrap();
    let steps = probe.mutations() - before;
    assert!(steps >= 8, "marker, pages, index, removal, marker: {steps}");

    let mut seen_interrupted = false;
    for crash_at in 0..=steps {
        let fs = MemFs::new();
        written(&files, &fs, &old);
        fs.crash_after(crash_at);
        let outcome = files.write(&fs, DIR, &new);
        let disk = fs.after_crash();

        // Whole files: everything the folder holds decodes.
        let pages_dir = format!("{DIR}/{PAGES}");
        let mut names = vec![INDEX.to_owned()];
        names.extend(
            disk.list(&pages_dir)
                .unwrap()
                .into_iter()
                .filter(|n| n.ends_with(".json"))
                .map(|n| format!("{PAGES}/{n}")),
        );
        for name in names {
            let bytes = disk.read(&format!("{DIR}/{name}")).unwrap();
            codec
                .decode(&bytes)
                .unwrap_or_else(|e| panic!("crash after {crash_at}: {name} is not whole: {e}"));
        }

        let loaded = files.read(&disk, DIR).unwrap_or_else(|e| {
            panic!("crash after {crash_at} of {steps} left an unreadable document: {e}")
        });
        if loaded.document != old && loaded.document != new {
            assert!(
                loaded.interrupted,
                "crash after {crash_at} of {steps}: a mixture of old and new said nothing"
            );
            seen_interrupted = true;
        }
        if outcome.is_ok() {
            assert_eq!(loaded.document, new);
            assert!(
                !loaded.interrupted && loaded.orphans.is_empty(),
                "a finished save is clean"
            );
        }
        // The next save finishes the job whatever happened before.
        files.write(&disk, DIR, &new).unwrap();
        let after = files.read(&disk, DIR).unwrap();
        assert_eq!(after.document, new, "after a crash at {crash_at}");
        assert!(!after.interrupted && after.orphans.is_empty());
    }
    assert!(
        seen_interrupted,
        "the simulation never produced a torn directory"
    );
}
