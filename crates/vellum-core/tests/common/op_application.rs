//! TEST DATA: shared deterministic regression/fuzzer oracle for the production op planner and edits.
#[path = "mutate.rs"]
mod mutate;
use vellum_core::ops::{self, Op, Transaction};
use vellum_types::NodeId;

pub fn check(data: &[u8]) {
    let bases = mutate::bases(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        if env!("CARGO_PKG_NAME").ends_with("-fuzz") {
            "../../../fixtures/documents"
        } else {
            "../../fixtures/documents"
        },
    ));
    let mut bytes = mutate::Bytes::new(data);
    let mut doc = bases[bytes.below(bases.len())].clone();
    let mut ids = mutate::TagIds::new("fuzz");
    for _ in 0..16 {
        let before = doc.clone();
        let mut batch = Vec::new();
        for _ in 0..1 + bytes.below(4) {
            if let Some(op) = mutate::one_op(&doc, &mut bytes, &mut ids) {
                batch.push(op);
            }
        }
        let invalid = bytes.byte() & 1 == 1;
        if invalid {
            batch.push(Op::Delete {
                id: NodeId::new("missing000").unwrap(),
            });
        }
        let origin = mutate::origin();
        let result = ops::plan(&doc, &Transaction::new(origin.clone(), batch), &mut ids);
        assert_eq!(doc, before, "planning mutated its input");
        if invalid {
            assert!(result.is_err(), "invalid tail must reject the entire batch");
        }
        if let Ok(plan) = result {
            ops::apply_edits(&mut doc, &plan.edits, origin.clone()).unwrap();
            assert_eq!(doc, plan.document);
            assert!(ops::check_document(&doc).is_empty());
            let mut undone = doc.clone();
            ops::apply_edits(&mut undone, &plan.inverse, origin).unwrap();
            assert_eq!(undone, before, "the complete batch has an exact inverse");
        }
    }
}
