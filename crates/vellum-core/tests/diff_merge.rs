//! Diff and merge over random documents (guide Prompt 15): every fixture document, edited at random through
//! the op layer on two branches. `common/mutate.rs` makes the edits from bytes; the fuzz target in `fuzz/`
//! uses the same code with coverage guidance, and `fuzz_smoke` here runs a fixed corpus on every `cargo test`.

#[path = "common/mutate.rs"]
mod mutate;

use std::path::Path;
use std::sync::OnceLock;

use proptest::prelude::*;
use vellum_types::Document;

use mutate::{case, check_diff, check_merge};

fn bases() -> &'static [Document] {
    static BASES: OnceLock<Vec<Document>> = OnceLock::new();
    BASES.get_or_init(|| {
        mutate::bases(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/documents"))
    })
}

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 16..400)
}

mod diff {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        /// apply(diff(old, new)) to old == new, for a document and an edited version of it.
        #[test]
        fn the_diff_leads_to_the_new_document(data in bytes()) {
            let (base, ours, theirs) = case(bases(), &data);
            check_diff(&base, &ours);
            check_diff(&ours, &base);
            // Two branches: renames, moves and removals the other never saw.
            check_diff(&ours, &theirs);
        }
    }

    /// Unrelated documents: the diff removes one and builds the other, ids colliding where they do.
    #[test]
    fn every_pair_of_fixture_documents() {
        for a in bases() {
            for b in bases() {
                check_diff(a, b);
            }
        }
    }
}

mod merge {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        /// merge(b, x, b) == x; merge(b, b, y) == y; the merge is valid; a clean merge commutes.
        #[test]
        fn merge_properties(data in bytes()) {
            let (base, ours, theirs) = case(bases(), &data);
            check_merge(&base, &ours, &theirs);
        }
    }

    /// The property above means little if every random pair conflicts: enough of them must merge cleanly.
    #[test]
    fn many_random_merges_are_clean() {
        let mut clean = 0;
        let total = 200;
        for seed in 0..total {
            let data = mutate::seeded(seed, 200);
            let (base, ours, theirs) = case(bases(), &data);
            if check_merge(&base, &ours, &theirs) {
                clean += 1;
            }
        }
        assert!(
            clean * 5 >= total,
            "only {clean} of {total} merges were clean"
        );
    }
}

/// The random edits are worth testing with: across a few hundred inputs, every kind of op the generator makes
/// is accepted many times, and most branches differ from their base.
#[test]
fn the_generator_makes_every_kind_of_change() {
    use std::collections::BTreeMap;
    let mut accepted = BTreeMap::new();
    let mut changed = 0;
    let total = 300u32;
    for seed in 0..total {
        let data = mutate::seeded(u64::from(seed), 300);
        let mut bytes = mutate::Bytes::new(&data);
        let base = &bases()[bytes.below(bases().len())];
        let edited = mutate::mutate_counting(base, &mut bytes, "o", 6, &mut accepted);
        if &edited != base {
            changed += 1;
        }
    }
    eprintln!("accepted: {accepted:?}; changed {changed} of {total}");
    let kinds = [
        "Update",
        "Insert",
        "Delete",
        "Move",
        "Reorder",
        "Replace",
        "SetVariables",
        "InsertPage",
        "DeletePage",
        "UpdatePage",
        "MovePage",
        "SetShell",
        "SetComponent",
        "UpdateComponent",
        "Group",
        "Ungroup",
        "Copy",
        "Flip",
        "CreateComponent",
    ];
    for kind in kinds {
        let n = accepted.get(kind).copied().unwrap_or(0);
        assert!(n >= 5, "{kind} was accepted {n} times: {accepted:?}");
    }
    assert!(
        changed * 10 >= total * 9,
        "only {changed} of {total} branches changed"
    );
}

/// What the fuzz target runs, on a fixed set of inputs, so `cargo test` covers it without nightly.
#[test]
fn fuzz_smoke() {
    for seed in 0..64 {
        let data = mutate::seeded(seed, 256);
        let (base, ours, theirs) = case(bases(), &data);
        check_diff(&base, &ours);
        check_diff(&ours, &theirs);
        check_merge(&base, &ours, &theirs);
    }
}
