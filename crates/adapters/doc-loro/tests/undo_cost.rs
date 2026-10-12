//! ADR-004's undo budget: a person's undo of a 200-node batch on a 10,000-node document stays under a frame
//! (16 ms). Timing needs a release build and a quiet machine, so it is ignored by default:
//!
//!     cargo test -p vellum-doc-loro --release --test undo_cost -- --ignored --nocapture

use std::time::{Duration, Instant};

use serde_json::json;
use vellum_contracts::fakes::SequentialIds;
use vellum_contracts::samples::{alice, bob, node, node_id, node_with, page, patch, tx};
use vellum_core::ops::{self, Op, Parent, Place};
use vellum_core::ports::DocStore;
use vellum_doc_loro::LoroDocStore;
use vellum_types::Document;

/// 100 frames of 99 rectangles each, on one page: 10,000 nodes.
fn document(frames: usize, per_frame: usize) -> Document {
    let mut doc = Document::new("TEST DATA: undo cost");
    let children = (0..frames)
        .map(|f| {
            let kids: Vec<_> = (0..per_frame)
                .map(|r| node("rect", &format!("r{f:03}x{r:03}")))
                .collect();
            node_with("frame", &format!("frame{f:03}"), &kids, json!({}))
        })
        .collect();
    doc.pages = vec![page("page00001", children)];
    doc
}

/// Moves 200 leaves and puts every fourth into another frame, as ADR-004's batch.
fn batch(round: usize) -> Vec<Op> {
    (0..200)
        .map(|i| {
            let id = node_id(&format!("r{:03}x{:03}", i % 100, i / 100 + round * 2));
            if i % 4 == 0 {
                Op::Move {
                    id,
                    to: Place {
                        parent: Parent::Node(node_id(&format!("frame{:03}", (i + 1) % 100))),
                        index: 0,
                    },
                }
            } else {
                Op::Update {
                    id,
                    patch: patch(json!({
                        "position": { "type": "absolute", "x": i as f64, "y": round as f64 }
                    })),
                }
            }
        })
        .collect()
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

#[test]
#[ignore = "timing: run in release, see the module comment"]
fn undo_of_a_200_node_batch_at_10k_nodes_fits_in_a_frame() {
    let doc = document(100, 99);
    let mut store = LoroDocStore::new(&doc, 1);
    let mut ids = SequentialIds::default();
    let (mut plan, mut apply, mut undo, mut redo) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for round in 0..7 {
        // Another person's work in between, so undo has later work to keep.
        ops::apply(
            &mut store,
            &mut ids,
            tx(
                &bob(),
                vec![vellum_contracts::samples::rename(
                    "r050x050",
                    "TEST DATA bob",
                )],
            ),
        )
        .expect("bob");
        let t = Instant::now();
        ops::plan(&store.snapshot(), &tx(&alice(), batch(round)), &mut ids).expect("plan");
        plan.push(t.elapsed());
        let t = Instant::now();
        ops::apply(&mut store, &mut ids, tx(&alice(), batch(round))).expect("the batch");
        apply.push(t.elapsed());
        let t = Instant::now();
        store.undo(&alice()).expect("undo");
        undo.push(t.elapsed());
        let t = Instant::now();
        store.redo(&alice()).expect("redo");
        redo.push(t.elapsed());
    }
    let (plan, apply, undo, redo) = (median(plan), median(apply), median(undo), median(redo));
    println!(
        "10,000 nodes, 200-node batch: plan alone {plan:?}, apply (plan + store) {apply:?}, undo {undo:?}, \
         redo {redo:?} (medians of 7)"
    );
    assert!(
        undo < Duration::from_millis(16),
        "undo took {undo:?}, over a 16 ms frame (ADR-004)"
    );
}
