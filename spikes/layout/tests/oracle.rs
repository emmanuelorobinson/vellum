//! Taffy against the Chrome layout oracle (guide Prompt 6, ADR-002). Every fixture in
//! `fixtures/layout-oracle/expected/` is laid out with Taffy through the spike's mapping and compared
//! with Chrome's boxes at 0.5 px. Mismatches that ADR-002 records are listed in `known-mismatches.txt`;
//! the test fails when a listed mismatch disappears as well as when a new one appears, so the list
//! stays exact. Needs no Chrome: the expected boxes are committed.

use std::collections::BTreeSet;
use std::path::PathBuf;

use spike_layout::compute;
use vellum_testkit::layout_oracle::{TOLERANCE_PX, compare, load_dir};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `fixture node-id` lines from known-mismatches.txt (text after `#` is the reason).
fn known() -> BTreeSet<String> {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("known-mismatches.txt"),
    )
    .unwrap();
    text.lines()
        .map(|l| l.split('#').next().unwrap().trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

#[test]
fn taffy_matches_the_chrome_oracle_except_known_mismatches() {
    let fixtures = load_dir(&repo().join("fixtures/layout-oracle/expected")).unwrap();
    assert!(fixtures.len() >= 40, "only {} fixtures", fixtures.len());
    let mut found = BTreeSet::new();
    let mut report = Vec::new();
    for fixture in &fixtures {
        let actual = compute(fixture);
        for m in compare(fixture, &actual, TOLERANCE_PX) {
            found.insert(format!("{} {}", fixture.fixture, m.id));
            report.push(format!("{}: {m}", fixture.fixture));
        }
    }
    let known = known();
    let new: Vec<_> = found.difference(&known).collect();
    let gone: Vec<_> = known.difference(&found).collect();
    assert!(
        new.is_empty() && gone.is_empty(),
        "new mismatches: {new:?}\nlisted but now matching: {gone:?}\nall mismatches:\n{}",
        report.join("\n")
    );
}

#[test]
fn compute_returns_a_box_for_every_visible_node() {
    let fixtures = load_dir(&repo().join("fixtures/layout-oracle/expected")).unwrap();
    for fixture in &fixtures {
        let actual = compute(fixture);
        for node in fixture.nodes().filter(|n| n.layout_box.is_some()) {
            assert!(
                actual.contains_key(&node.id),
                "{} {}",
                fixture.fixture,
                node.id
            );
        }
    }
}
