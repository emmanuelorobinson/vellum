//! `DocumentTarget`.

use vellum_core::guard::WriteError;
use vellum_core::ports::{DocumentTarget, TargetError};
use vellum_types::Document;

use crate::samples::{self, alice, rename, tx};

/// A target opened on a document has it; a valid transaction lands whole and an invalid one changes nothing;
/// a save leaves nothing unsaved.
///
/// `make` opens a target on the document it is given, as if a file held it.
pub fn run<T: DocumentTarget>(mut make: impl FnMut(&Document) -> T) {
    let doc = samples::document();
    let mut target = make(&doc);
    assert_eq!(
        target.snapshot().expect("snapshot"),
        doc,
        "DocumentTarget: a fresh target must hold the document it was opened on"
    );
    assert!(
        !target.is_dirty().expect("dirty"),
        "DocumentTarget: nothing is unsaved before anything changed"
    );

    let applied = target
        .transact(tx(&alice(), vec![rename("rect000001", "Renamed")]))
        .expect("DocumentTarget: a valid transaction must apply");
    assert!(
        applied
            .changes
            .updated
            .iter()
            .any(|n| n.as_str() == "rect000001"),
        "DocumentTarget: the change set must name the node that changed"
    );
    let after = target.snapshot().expect("snapshot");
    assert_eq!(
        samples::find(&after, "rect000001").map(|n| n.name()),
        Some("Renamed"),
        "DocumentTarget: the snapshot must show an applied transaction"
    );

    // A two-op transaction whose second op fails leaves the first one out too.
    let refused = target.transact(tx(
        &alice(),
        vec![rename("rect000002", "Half"), rename("missing001", "Nobody")],
    ));
    assert!(
        matches!(refused, Err(TargetError::Write(WriteError::Invalid(_)))),
        "DocumentTarget: an invalid transaction must be refused as invalid, got {refused:?}"
    );
    assert_eq!(
        target.snapshot().expect("snapshot"),
        after,
        "DocumentTarget: a refused transaction must change nothing"
    );

    target.save().expect("DocumentTarget: save");
    assert!(
        !target.is_dirty().expect("dirty"),
        "DocumentTarget: nothing is unsaved after a save"
    );
    assert_eq!(target.snapshot().expect("snapshot"), after);
}
