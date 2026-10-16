//! `DocumentTarget`.

use vellum_core::guard::{AgentControl, LeaseChange, LeaseRefused, Rejection, WriteError};
use vellum_core::lease::LeaseError;
use vellum_core::ops::Transaction;
use vellum_core::ports::{DocumentTarget, TargetError, TurnUndone};
use vellum_types::{Actor, Document, Origin, TurnId};

use crate::samples::{self, alice, rename, rose, teal, tx};

/// A target opened on a document has it; a valid transaction lands whole and an invalid one changes nothing;
/// a save leaves nothing unsaved; Pause all and Stop reach its Guard; Undo this turn takes back only that turn.
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

    // Leases live where the Guard does: a claim holds the frame against other agents until it is released.
    let frame = samples::node_id("frame00001");
    target
        .lease(&frame, &teal(), LeaseChange::Claim)
        .expect("DocumentTarget: an agent can claim a free frame");
    let taken = target.lease(&frame, &rose(), LeaseChange::Claim);
    assert!(
        matches!(taken, Err(TargetError::Lease(LeaseRefused::Lease(LeaseError::LeasedBy(ref by)))) if *by == teal()),
        "DocumentTarget: a frame another agent holds cannot be claimed, got {taken:?}"
    );
    let blocked = target.transact(tx(
        &Actor::Agent { agent: rose() },
        vec![rename("rect000001", "Not yours")],
    ));
    assert!(
        matches!(blocked, Err(TargetError::Write(WriteError::Rejected(_)))),
        "DocumentTarget: the lease must hold against another agent's write, got {blocked:?}"
    );
    let not_frame = target.lease(&samples::node_id("rect000003"), &teal(), LeaseChange::Claim);
    assert!(
        matches!(
            not_frame,
            Err(TargetError::Lease(LeaseRefused::NotAFrame(_)))
        ),
        "DocumentTarget: only frames are leased, got {not_frame:?}"
    );
    target
        .lease(&frame, &teal(), LeaseChange::Release)
        .expect("DocumentTarget: the holder can release");
    target
        .lease(&frame, &rose(), LeaseChange::Claim)
        .expect("DocumentTarget: a released frame can be claimed again");
    target
        .lease(&frame, &rose(), LeaseChange::Release)
        .expect("DocumentTarget: release");
    assert_eq!(
        target.snapshot().expect("snapshot"),
        after,
        "DocumentTarget: leases are session state and never change the document"
    );

    target.save().expect("DocumentTarget: save");
    assert!(
        !target.is_dirty().expect("dirty"),
        "DocumentTarget: nothing is unsaved after a save"
    );
    assert_eq!(target.snapshot().expect("snapshot"), after);

    controls(&mut target);
    undo_turn(&mut target);
}

/// Pause all and Stop reach the Guard where it lives (guide Invariants 4, 12).
fn controls<T: DocumentTarget>(target: &mut T) {
    let write = |target: &mut T, agent| {
        target.transact(tx(
            &Actor::Agent { agent },
            vec![rename("rect000002", "Agent's")],
        ))
    };
    target
        .control(&AgentControl::Stop(teal()))
        .expect("DocumentTarget: control");
    let refused = write(target, teal());
    assert!(
        matches!(
            refused,
            Err(TargetError::Write(WriteError::Rejected(Rejection::Stopped { ref agent }))) if *agent == teal()
        ),
        "DocumentTarget: a stopped agent's next write must be rejected, got {refused:?}"
    );
    write(target, rose()).expect("DocumentTarget: Stop names one agent; another still writes");
    target
        .control(&AgentControl::Resume(teal()))
        .expect("DocumentTarget: control");
    write(target, teal()).expect("DocumentTarget: a resumed agent writes again");

    target
        .control(&AgentControl::PauseAll)
        .expect("DocumentTarget: control");
    let refused = write(target, rose());
    assert!(
        matches!(
            refused,
            Err(TargetError::Write(WriteError::Rejected(
                Rejection::AllPaused
            )))
        ),
        "DocumentTarget: Pause all must reject every agent's next write, got {refused:?}"
    );
    target
        .transact(tx(&alice(), vec![rename("rect000002", "Mine")]))
        .expect("DocumentTarget: Pause all never blocks a person");
    target
        .control(&AgentControl::ResumeAll)
        .expect("DocumentTarget: control");
    write(target, rose()).expect("DocumentTarget: Resume all lifts the pause");
}

/// "Undo this turn" reverts only the transactions whose origin carries that turn (guide Invariant 3).
fn undo_turn<T: DocumentTarget>(target: &mut T) {
    let turn = TurnId::new("01JA0000000000000000000T39").expect("a ULID");
    let in_turn = |ops| {
        Transaction::new(
            Origin::new(Actor::Agent { agent: teal() }).in_turn(turn.clone()),
            ops,
        )
    };
    let before = target.snapshot().expect("snapshot");
    let name = |doc: &Document, id| samples::find(doc, id).map(|n| n.name().to_owned());
    target
        .transact(in_turn(vec![rename("rect000003", "Teal's turn")]))
        .expect("DocumentTarget: the turn's change");
    target
        .transact(tx(
            &Actor::Agent { agent: rose() },
            vec![rename("rect000001", "Rose's")],
        ))
        .expect("another agent's change");
    let undone = target
        .undo_turn(&turn, &alice())
        .expect("DocumentTarget: undo_turn");
    assert!(
        matches!(undone, TurnUndone::Undone(ref changes) if changes.updated.iter().any(|n| n.as_str() == "rect000003")),
        "DocumentTarget: a turn nothing later depends on is undone outright, got {undone:?}"
    );
    let now = target.snapshot().expect("snapshot");
    assert_eq!(
        name(&now, "rect000003"),
        name(&before, "rect000003"),
        "DocumentTarget: undo_turn must take the turn's change back"
    );
    assert_eq!(
        name(&now, "rect000001").as_deref(),
        Some("Rose's"),
        "DocumentTarget: undo_turn must keep another actor's change"
    );
    assert_eq!(
        target.undo_turn(&turn, &alice()).expect("undo_turn"),
        TurnUndone::Nothing,
        "DocumentTarget: an undone turn has nothing left to undo"
    );
}
