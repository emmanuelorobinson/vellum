//! ADR-004 measurements: the spike document as a Loro movable tree at 1k to 50k nodes.
//!
//!   cargo run --release -p spike-loro --bin loro-bench -- [out.json] [--runs N]
//!
//! Per size: build, save (snapshot and state-only snapshot), load, first full read, live heap after
//! load, the same state as JSON (size, stringify, parse), and a 200-node batch: local apply and commit,
//! with an UndoManager attached, update size, and a remote peer importing it. Medians of N runs.
//! Loro decodes a snapshot lazily: `loadMs` is the import alone and `firstFullReadMs` the first
//! `get_deep_value` after it; every batch number is taken on a fully decoded document.
//! Wall-clock timing lives only in this binary, never in the library.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Instant;

use loro::{ExportMode, UndoManager};
use spike_loro::{
    BATCH_SIZE, apply_edits, from_document, load, node_count, pick_batch, save, to_json,
    to_json_pretty,
};
use spike_scene::generate;

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_add(new as isize - layout.size() as isize, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn ms<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let out = f();
    (out, start.elapsed().as_secs_f64() * 1000.0)
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs[xs.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let runs = args
        .iter()
        .position(|a| a == "--runs")
        .map_or(5, |i| args[i + 1].parse().expect("--runs N"));
    let out = args.first().filter(|a| !a.starts_with("--")).cloned();
    let mut json = String::from("[\n");
    let sizes = [1_000, 5_000, 10_000, 25_000, 50_000];
    for (si, &n) in sizes.iter().enumerate() {
        let scene = generate(n);
        let mut t: [Vec<f64>; 12] = Default::default();
        let mut sizes_bytes = (0, 0, 0, 0);
        let mut heap = 0isize;
        let mut heap_full = 0isize;
        let mut pretty_bytes = 0usize;
        for run in 0..runs {
            let (doc, build) = ms(|| from_document(&scene));
            assert_eq!(node_count(&doc), scene.node_count());
            let (snapshot, export) = ms(|| save(&doc));
            let (state_only, state_export) =
                ms(|| doc.export(ExportMode::StateOnly(None)).expect("state only"));
            drop(doc);

            let before = LIVE.load(Ordering::Relaxed);
            let (loaded, import) = ms(|| load(&snapshot).expect("load"));
            heap = LIVE.load(Ordering::Relaxed) - before;
            let (_, first_read) = ms(|| loaded.get_deep_value());
            heap_full = LIVE.load(Ordering::Relaxed) - before;
            let (text, stringify) = ms(|| to_json(&loaded));
            pretty_bytes = to_json_pretty(&loaded).len();
            let (_, parse) =
                ms(|| serde_json::from_str::<serde_json::Value>(&text).expect("parse"));

            let vv = loaded.oplog_vv();
            let remote = load(&snapshot).expect("load");
            remote.set_peer_id(2).expect("peer");
            // Decode the remote's state first: a lazily loaded doc pays that on its first import.
            let _ = remote.get_deep_value();
            let _ = loaded.get_deep_value();
            let edits = pick_batch(&loaded, run as u64);
            let (_, batch) = ms(|| apply_edits(&loaded, &edits));
            let update = loaded.export(ExportMode::updates(&vv)).expect("update");
            let (_, remote_import) = ms(|| remote.import(&update).expect("import"));
            // A second batch on the same peers: the remote's history is decoded now.
            let vv2 = loaded.oplog_vv();
            let edits2 = pick_batch(&loaded, run as u64 + 100);
            apply_edits(&loaded, &edits2);
            let update2 = loaded.export(ExportMode::updates(&vv2)).expect("update");
            let (_, remote_import2) = ms(|| remote.import(&update2).expect("import"));
            assert_eq!(remote.get_deep_value(), loaded.get_deep_value());

            let undoable = load(&snapshot).expect("load");
            undoable.set_peer_id(3).expect("peer");
            let _ = undoable.get_deep_value();
            let mut undo = UndoManager::new(&undoable);
            let edits = pick_batch(&undoable, run as u64);
            let (_, batch_undo) = ms(|| apply_edits(&undoable, &edits));
            let (_, undo_ms) = ms(|| undo.undo().expect("undo"));

            for (k, v) in [
                build,
                export,
                state_export,
                import,
                first_read,
                stringify,
                parse,
                batch,
                remote_import,
                batch_undo,
                undo_ms,
                remote_import2,
            ]
            .into_iter()
            .enumerate()
            {
                t[k].push(v);
            }
            sizes_bytes = (snapshot.len(), state_only.len(), text.len(), update.len());
        }
        let [
            build,
            export,
            state_export,
            import,
            first_read,
            stringify,
            parse,
            batch,
            remote,
            batch_undo,
            undo,
            remote2,
        ] = t.map(median);
        let mb = |b: usize| b as f64 / 1_000_000.0;
        eprintln!(
            "{n:>6} nodes: build {build:.1} ms, save {export:.1} ms ({:.2} MB), load {import:.1} ms, \
             heap {:.1} MB, json {:.2} MB, batch {batch:.2} ms, remote {remote:.2} ms, remote again {remote2:.2} ms",
            mb(sizes_bytes.0),
            heap as f64 / 1e6,
            mb(sizes_bytes.2)
        );
        let _ = writeln!(
            json,
            "  {{\"nodes\": {n}, \"runs\": {runs}, \"buildMs\": {build:.2}, \"saveMs\": {export:.2}, \
             \"snapshotBytes\": {}, \"stateOnlySaveMs\": {state_export:.2}, \"stateOnlyBytes\": {}, \
             \"loadMs\": {import:.2}, \"firstFullReadMs\": {first_read:.2}, \"heapAfterLoadBytes\": {heap}, \"heapAfterFullReadBytes\": {heap_full}, \
             \"jsonBytes\": {}, \"jsonPrettyBytes\": {pretty_bytes}, \"jsonStringifyMs\": {stringify:.2}, \"jsonParseMs\": {parse:.2}, \
             \"batchNodes\": {BATCH_SIZE}, \"batchApplyMs\": {batch:.3}, \"batchApplyWithUndoMs\": {batch_undo:.3}, \
             \"undoBatchMs\": {undo:.3}, \"batchUpdateBytes\": {}, \"batchRemoteImportMs\": {remote:.3}, \"secondBatchRemoteImportMs\": {remote2:.3}}}{}",
            sizes_bytes.0,
            sizes_bytes.1,
            sizes_bytes.2,
            sizes_bytes.3,
            if si + 1 < sizes.len() { "," } else { "" }
        );
    }
    json.push_str("]\n");
    match out {
        Some(path) => std::fs::write(&path, json).expect("write results"),
        None => print!("{json}"),
    }
}
