//! `Clock` and `IdGen`.

use std::collections::HashSet;

use vellum_core::ports::{Clock, IdGen};

/// A clock never goes back.
pub fn run_clock(clock: &dyn Clock) {
    let mut last = clock.now();
    for call in 0..100 {
        let now = clock.now();
        assert!(
            now >= last,
            "Clock: the time went back on call {call}: {last:?} then {now:?}"
        );
        last = now;
    }
}

/// Ids are ten characters, valid for their type, and never repeat, across every kind of id.
pub fn run_id_gen(ids: &mut dyn IdGen) {
    let mut seen = HashSet::new();
    for round in 0..200 {
        let minted = [
            ids.node_id().to_string(),
            ids.page_id().to_string(),
            ids.component_id().to_string(),
            ids.variable_id().to_string(),
            ids.style_id().to_string(),
        ];
        for id in minted {
            assert_eq!(
                id.len(),
                10,
                "IdGen: {id:?} (round {round}) is not ten characters"
            );
            assert!(
                seen.insert(id.clone()),
                "IdGen: {id:?} was minted twice (round {round})"
            );
        }
    }
}
