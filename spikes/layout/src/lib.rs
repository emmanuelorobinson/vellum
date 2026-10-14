//! Spike C's oracle entry point, now delegated to the production adapter (Prompt 18).
//! ADR-002 and Git retain the original experiment; keeping another semantic mapper would drift.
use std::collections::BTreeMap;
use vellum_core::ports::{DirtySet, LayoutEngine};
use vellum_testkit::layout_oracle::{LayoutBox, OracleFixture, layout_tree};
/// Production layout of the oracle's inputs, returned with the oracle's index-path keys.
pub fn compute(fixture: &OracleFixture) -> BTreeMap<String, LayoutBox> {
    vellum_layout_taffy::TaffyLayout::new()
        .layout(
            &layout_tree(fixture),
            &DirtySet::all(),
            &vellum_contracts::fakes::FixedMetricText::new(),
        )
        .boxes
        .into_iter()
        .map(|(id, b)| {
            (
                id.to_string().replace('/', "."),
                LayoutBox::new(b.x, b.y, b.width, b.height),
            )
        })
        .collect()
}
