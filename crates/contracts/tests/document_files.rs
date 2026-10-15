//! `DocumentFiles`: a document kept at a path, as one file or as a folder (guide Prompt 24).

use vellum_contracts::fakes::{FakeJsonCodec, MemFs};
use vellum_contracts::{samples, suites};
use vellum_core::files::{
    LARGE_FILE_BYTES, LARGE_FILE_NODES, SingleFile, count_nodes, suggest_multi_file,
};
use vellum_core::ports::{DocumentFiles, FileError, FsError};
use vellum_types::Document;

#[test]
fn a_single_file_passes_the_document_files_suite() {
    let codec = FakeJsonCodec;
    suites::document_files::run(
        &SingleFile::new(&codec),
        &MemFs::new(),
        &[samples::document(), Document::new("TEST DATA: empty")],
    );
}

#[test]
fn a_single_file_is_written_whole_or_not_at_all() {
    let codec = FakeJsonCodec;
    let files = SingleFile::new(&codec);
    let old = Document::new("TEST DATA: old");
    let new = samples::document();

    let probe = MemFs::new();
    files.write(&probe, "doc.vellum", &old).unwrap();
    let before = probe.mutations();
    files.write(&probe, "doc.vellum", &new).unwrap();
    let steps = probe.mutations() - before;

    for crash_at in 0..=steps {
        let fs = MemFs::new();
        files.write(&fs, "doc.vellum", &old).unwrap();
        fs.crash_after(crash_at);
        let _ = files.write(&fs, "doc.vellum", &new);
        let disk = fs.after_crash();
        let loaded = files.read(&disk, "doc.vellum").expect("always readable");
        assert!(
            loaded.document == old || loaded.document == new,
            "crash after {crash_at} of {steps}"
        );
        assert!(!loaded.interrupted, "one file is never torn");
    }
}

#[test]
fn reading_what_is_not_there_or_not_a_document_says_so() {
    let codec = FakeJsonCodec;
    let files = SingleFile::new(&codec);
    let fs = MemFs::new();
    assert!(matches!(
        files.read(&fs, "nope.vellum"),
        Err(FileError::Fs(FsError::NotFound(_)))
    ));
    fs.seed("junk.vellum", b"this is not json");
    assert!(matches!(
        files.read(&fs, "junk.vellum"),
        Err(FileError::Codec(_))
    ));
}

#[test]
fn the_large_file_suggestion_follows_adr_004() {
    assert_eq!((LARGE_FILE_NODES, LARGE_FILE_BYTES), (10_000, 4_000_000));
    assert!(!suggest_multi_file(0, 0));
    assert!(!suggest_multi_file(9_999, 3_999_999));
    assert!(suggest_multi_file(10_000, 0), "10,000 nodes");
    assert!(suggest_multi_file(5, 4_000_000), "4 MB of JSON");
    assert!(suggest_multi_file(50_000, 40_000_000));
}

#[test]
fn nodes_are_counted_on_pages_and_inside_components() {
    let doc = samples::document();
    let on_pages: usize = doc
        .pages
        .iter()
        .map(|p| p.children.iter().map(|n| 1 + descendants(n)).sum::<usize>())
        .sum();
    assert!(on_pages > 0);
    assert_eq!(count_nodes(&doc), on_pages, "the sample has no components");
    assert_eq!(count_nodes(&Document::new("TEST DATA: empty")), 0);
}

fn descendants(node: &vellum_types::Node) -> usize {
    node.children().iter().map(|c| 1 + descendants(c)).sum()
}
