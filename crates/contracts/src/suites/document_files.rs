//! `DocumentFiles`.

use vellum_core::ports::{DocumentFiles, FileSystem};
use vellum_types::Document;

/// A document written to a path comes back equal, writing it again changes nothing on disk, a change rewrites
/// only what changed, and a path with nothing or junk at it is an error, never an empty document.
///
/// `docs` sit on the codec's number grid, so reading what was written gives the document back. `fs` has nothing
/// at the paths `contract-suite-*.vellum`.
pub fn run(files: &dyn DocumentFiles, fs: &dyn FileSystem, docs: &[Document]) {
    assert!(!files.id().is_empty(), "DocumentFiles: the id is empty");
    assert!(
        !docs.is_empty(),
        "DocumentFiles: the suite needs a document"
    );
    assert!(
        files.read(fs, "contract-suite-missing.vellum").is_err(),
        "DocumentFiles: reading a path with nothing at it must fail"
    );

    for (n, doc) in docs.iter().enumerate() {
        let path = format!("contract-suite-{n}.vellum");
        let saved = files
            .write(fs, &path, doc)
            .unwrap_or_else(|e| panic!("DocumentFiles: document {n} could not be written: {e}"));
        assert!(
            !saved.written.is_empty(),
            "DocumentFiles: a first write wrote nothing"
        );
        let loaded = files
            .read(fs, &path)
            .unwrap_or_else(|e| panic!("DocumentFiles: document {n} could not be read: {e}"));
        assert_eq!(
            &loaded.document, doc,
            "DocumentFiles: document {n} changed in a round trip"
        );
        assert!(
            loaded.orphans.is_empty() && !loaded.interrupted,
            "DocumentFiles: a clean write reads back with leftovers"
        );

        let again = files.write(fs, &path, doc).expect("the second write");
        assert!(
            again.written.is_empty() && again.removed.is_empty(),
            "DocumentFiles: writing the same document again touched {:?}",
            again.written
        );

        let mut changed = doc.clone();
        changed.meta.name = format!("{} (changed)", doc.meta.name);
        let rewrite = files.write(fs, &path, &changed).expect("a changed write");
        assert!(
            !rewrite.written.is_empty(),
            "DocumentFiles: a changed document wrote nothing"
        );
        assert_eq!(
            files.read(fs, &path).expect("read changed").document,
            changed,
            "DocumentFiles: a changed document did not come back changed"
        );
    }
}
