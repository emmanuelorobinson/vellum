# ADR-004: Loro behind DocStore; directory-codec threshold

- **Status:** Accepted
- **Date:** 2026-10-12
- **Written by:** Prompt 5 (Spike B). It also records the QuickJS-in-WASM numbers the guide asks Prompt 5 for, as
  input to ADR-005 (Prompt 25).
- **Evidence:** `spikes/engine/` (code and tests), `spikes/engine/results/{loro,loro-wasm,script}.json` (every number
  below), `docs/runbooks/engine-spike.md` (how to rebuild and re-measure).

## Context

ARCHITECTURE §4.2 puts Loro behind the `DocStore` port: one movable tree for the node hierarchy, maps for properties,
undo, history and binary sync from Loro, nothing of Loro visible outside the adapter. ARCHITECTURE open question 3
asks for load time and memory on 10k+ node files and when the directory codec (`codec-json-dir`, Prompt 24) should be
suggested (DESIGN.md S06, "never blocks"). ADR-001 put the engine and the renderer in one emscripten module, so Loro
also has to work, and be fast enough, on `wasm32-unknown-emscripten`.

The spike stores the renderer spike's generated documents (`spike_scene::generate`, frames, rects, ellipses,
polygons, text and images; 1,000 to 50,000 nodes) as one `LoroTree` with fractional indexes on (children are
ordered, as layers are). Each scene node is one tree node; its properties live in the node's metadata map, paints as
one compact string each. Prompt 7's real types carry more fields, so absolute sizes will grow; the ratios and the
scaling are what this ADR relies on.

## Numbers

Apple M5 Pro, macOS 27.0.1, loro 1.16.2, Rust 1.99 release (Cargo's default profile, as the product workspace).
Medians of 7 runs. WASM is `wasm32-unknown-emscripten` (emsdk 4.0.6) in Node 24's V8.

Loro decodes snapshots lazily: "load" is the import alone, "first full read" the first `get_deep_value` after it, so
**load + first full read** is the cost of opening a document. Batch numbers are taken on a fully decoded document.

### Save, load and memory (10k is the guide's size; the rest show the scaling)

| Nodes | Snapshot | State-only snapshot | Save native / WASM | Open (load + first read) native / WASM | Heap after open (native) |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 0.10 MB | 0.06 MB | 1.7 / 6.2 ms | 2.8 / 7.4 ms | 0.4 MB |
| 5,000 | 0.45 MB | 0.28 MB | 8.1 / 18.3 ms | 14.5 / 30.3 ms | 1.9 MB |
| **10,000** | **0.89 MB** | **0.55 MB** | **16.0 / 35.1 ms** | **29.0 / 55.8 ms** | **3.8 MB** |
| 25,000 | 2.23 MB | 1.37 MB | 41.6 / 75.0 ms | 81.0 / 121.2 ms | 8.3 MB |
| 50,000 | 4.46 MB | 2.75 MB | 97.5 / 169.6 ms | 239.0 / 333.5 ms | 13.3 MB |

Building the 10k tree in one transaction takes 19.7 ms natively and 43.4 ms in WASM. The heap column counts every
byte the Rust allocator holds after the open (a counting global allocator, `loro-bench`); WASM linear memory after a
whole measurement run (several documents alive at once) was 51 MB at 10k, an upper bound only.

### A 200-node batch

One transaction that sets `x` and `y` on 200 leaves and moves every fourth into another top-level frame.

| Nodes | Apply native / WASM | With an UndoManager attached | Undo the batch | Update size | Remote import, first / later |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 0.44 / 1.00 ms | 0.50 ms | 1.9 ms | 6.4 KB | 1.0 / 0.16 ms |
| 5,000 | 0.52 / 1.33 ms | 0.65 ms | 6.7 ms | 6.6 KB | 3.5 / 0.50 ms |
| **10,000** | **0.63 / 0.97 ms** | **0.67 ms** | **13.1 ms** | **6.7 KB** | **6.3 / 0.54 ms** |
| 25,000 | 0.96 / 1.14 ms | 1.07 ms | 36.6 ms | 6.7 KB | 15.8 / 0.70 ms |
| 50,000 | 1.87 / 1.95 ms | 2.02 ms | 108.8 ms | 6.7 KB | 38.1 / 1.40 ms |

Applying a batch barely grows with the document. Two things do grow with it:

- **The first import after a load** (6.3 ms at 10k natively, 11.0 ms in WASM) decodes the op log; later imports cost
  about 0.5 ms. A peer pays it once per load.
- **Undo** of the batch: 13 ms at 10k, 109 ms at 50k, while applying it costs under 2 ms. Undo is one of Vellum's
  product invariants (3, "Undo is yours"), so Prompt 14 must measure per-actor undo on 10k+ documents and keep it
  under a frame at 10k (pre-computing inverses in the adapter if Loro's undo stays proportional to the document).

### The same state as JSON (stand-in for the `.vellum` file)

`serde_json` over Loro's deep value. The real codec (Prompt 9) has its own layout; these are proportions.

| Nodes | JSON compact | JSON 2-space | Stringify (compact) | Parse |
|---:|---:|---:|---:|---:|
| 1,000 | 0.19 MB | 0.38 MB | 3.0 ms | 0.5 ms |
| 10,000 | 1.90 MB | 3.84 MB | 31.8 ms | 5.0 ms |
| 25,000 | 4.82 MB | 9.64 MB | 89.2 ms | 13.1 ms |
| 50,000 | 9.74 MB | 19.40 MB | 278.7 ms | 34.9 ms |

### Determinism across targets

For the same generated document, native and WASM Loro produce **equal content in a different order**: the deep
value's map keys come out in a different order, and the snapshot bytes differ (95,792 vs 94,896 bytes at 1k). Loro's
binary format and its iteration order are not stable across targets. This does not break invariant 5 as long as the
codec never relies on either: `.vellum` is written from `DocumentData` with keys in a fixed order (ARCHITECTURE §4.3),
never from Loro's iteration order, and Loro bytes are never the file.

### Loro on the browser targets

loro-internal turns on getrandom's `js` backend and wasm-bindgen for `wasm32` targets other than emscripten and WASI
(`cfg(all(target_arch = "wasm32", not(target_os = "emscripten"), not(target_os = "wasi")))`). On
`wasm32-unknown-unknown` the module therefore needs wasm-bindgen's JS glue; on emscripten, ADR-001's target, it needs
neither, and the spike's module imports only emscripten's own runtime. The emscripten module holding Loro and the spike
scene generator is 3.05 MB raw, 1.13 MB gzip; Loro's share alone was not separated.

## QuickJS in WASM (input for ADR-005)

rquickjs 0.14.0 with a toy document API (`doc.create`, `doc.set`), a memory limit, a stack limit and an interrupt
budget counted in interrupt-handler calls rather than time, so a stopped script stops at the same point everywhere.
The 500-op script makes 100 creates and 400 sets.

| | Native | wasm32 (Node) |
|---|---:|---:|
| 500-op script, new runtime each time (median / p95) | 0.15 / 0.22 ms | 0.22 / 0.71 ms |
| 500-op script on a kept engine (median / p95) | 0.07 / 0.08 ms | 0.11 / 0.14 ms |

| Size added | Raw | gzip -9 |
|---|---:|---:|
| to `vellum-wasm` (`wasm32-unknown-unknown`, today's target; the crate alone is 357 bytes) | 1,277 KB | 444 KB |
| to Renderer B's emscripten module (with Parley), ADR-001's editor module | 946 KB | 356 KB |

On `wasm32-unknown-unknown` the module imports exactly one host function, `env.__rquickjs_host_now_us` (rquickjs's
clock; Prompt 25 decides what it returns, since scripts must not read the wall clock). On emscripten rquickjs has no
shipped bindings, so the build generates them with bindgen against emscripten's sysroot. The sandbox tests show no
host globals (`require`, `process`, `std`, `os`, `fetch`, `WebAssembly`, `setTimeout`, ...), an endless loop stopped
by the budget, allocation past an 8 MB limit failing, deep recursion as a catchable stack error, and a kept engine
running the same script again (each run gets its own function scope). The escape corpus is Prompt 25's.

## Decision

1. **Keep Loro behind `DocStore`** (loro 1.16.x, movable tree with fractional indexes, properties in node metadata
   maps). At the guide's 10k nodes it opens in 29 ms natively and 56 ms in WASM, holds about 4 MB, saves a 0.9 MB
   snapshot and applies a 200-node batch in under 1 ms on both. Nothing measured asks for an alternative.
   Automerge and Yrs were not measured; they stay the named fallbacks in ARCHITECTURE §3.1 if Prompt 14 cannot meet the
   undo target below.
2. **The adapter owns ordering.** Nothing outside `doc-loro` may depend on Loro's map iteration order or snapshot
   bytes; the codec writes from `DocumentData` in a fixed key order.
3. **Directory-codec threshold.** S06 suggests the directory format when a document reaches **10,000 nodes or 4 MB of
   `.vellum` JSON, whichever comes first**, and never blocks. At that size one save serializes the whole file in
   about 32 ms (two frames) and Git diffs and merges one file of about 4 MB; the directory codec rewrites only the
   pages that changed. Below it, a single file stays the default. Prompt 24 re-measures with the real codec and may
   move the numbers, not the rule.
4. **Load lazily, then warm up.** Opening a document should decode state off the frame path; a peer's first remote
   import after a load costs about 10x a later one, so the session warms Loro (one full read) before the first
   collaborative update arrives.

## Costs and risks accepted

- Undo cost proportional to the document (13 ms per 200-node batch at 10k, 109 ms at 50k). Prompt 14 must measure
  per-actor undo and fix or work around it; if it cannot get under 16 ms at 10k, this ADR is revisited.
- Loro's binary output is not byte-stable across targets. Fine for sync and recovery (content is equal), never usable
  as a file format or a checksum.
- On `wasm32-unknown-unknown`, Loro brings wasm-bindgen. Another reason to move `vellum-wasm` to emscripten (ADR-001).
- QuickJS adds about 0.36 to 0.44 MB gzip to the editor module.

## Not measured

- M1 and Windows (only the M5 Pro).
- Concurrent edits from several peers, long histories (only one or two batches of history), and shallow snapshots.
- Memory inside the browser (only Node's V8 and the native allocator).
- Loro's own WASM size without the scene generator.
- Automerge and Yrs.

## Revisit when

- Prompt 14 cannot keep a 200-node per-actor undo under 16 ms at 10,000 nodes.
- A Loro major release, or a format change that breaks importing older snapshots (guide §5: a Loro major gets its own
  reviewed prompt).
- Real `.vellum` files (Prompt 9 codec, Prompt 7 types) are more than twice the sizes above at the same node count:
  then re-derive the threshold.

## Reference reading

- `refs/loro/context/tree-events.md` and `tree-checkout-window.md`: tree events describe only the alive tree, and
  checkout or import walks a window of ops from the smallest lamport the two versions disagree on. Vellum keeps
  Loro's movable tree for exactly that (moves and reparenting merge correctly) and takes from it that the adapter must
  rebuild its own view from events, not assume a `Create` per node. Nothing was ported.
- `refs/loro/docs/encoding.md`: snapshots carry the op log, the state and a shallow-root state in one envelope with a
  checksum. That is why the snapshot (0.89 MB at 10k) is larger than the state-only export (0.55 MB), and why Vellum
  keeps Loro bytes for sync and recovery only, never as the file.
- `refs/rquickjs/core/src/runtime/{base,raw}.rs` and `wasm-test/`: memory limit, max stack size (mandatory on wasm32,
  where the default wraps), and an interrupt handler; the wasm test drives a C-ABI cdylib from a host. The spike does
  the same and changes the interrupt from a wall-clock deadline to a call budget, because Vellum's logic must not read
  the clock. rquickjs is MIT; nothing was copied.

## Prompt 14: the Loro store, and undo outside Loro

- **Date:** 2026-10-12
- **Written by:** Prompt 14 (DocStore on Loro).
- **Evidence:** `crates/adapters/doc-loro/` (`src/backend.rs`, tests), `crates/vellum-core/src/history/`,
  `crates/contracts/src/suites/doc_store.rs` (`undo_keeps_later_work`, `undo_turn*`, `undo_is_yours`),
  `crates/adapters/doc-loro/tests/undo_cost.rs` (the timing below).

### Decision

1. **Layout in Loro.** One movable tree (`nodes`, fractional indexes, jitter 0). Its roots are the pages in order
   and one root per component; every document node is one tree node whose meta map holds each property (children
   aside) as a JSON string, so concurrent edits to different properties of one node merge. What no edit can change
   (meta, imports, themes, styles, fonts) is one JSON string in the map `document`; variables are JSON strings in
   the map `variables`. Nothing Loro is public: `LoroDocStore` hands out and takes our types only.
2. **Same document, same genesis.** Each store writes the document it was opened from as a fixed peer derived from
   the document's content (FNV-1a of its JSON, top bit set), then switches to its own peer (which must be below
   2^63). Two stores opened from the same file share that first change byte for byte, so they sync without
   duplicating the document; updates whose genesis peer differs are refused (`SyncError::DifferentDocument`).
3. **Undo does not use Loro's `UndoManager`.** Loro's undo is per peer, while Vellum's is per actor (several people
   and agents write through one store) and per turn; and it grows with the document (13 ms per batch at 10k, 109 ms
   at 50k, above). `vellum_core::history` (pure, shared by every store) records each commit's inverse and a
   footprint (properties set one by one, nodes made, removed and moved, containers whose children changed,
   variables, components). Undo replays the inverse adjusted to the document as it is: a property someone changed
   since keeps their value, a node someone built on is not removed, a node someone moved since is not moved back,
   a variable or component someone changed since is not put back. Changes from other stores are recorded as the
   state before and after, count as later work, and are never undone locally.
4. **Undo this turn.** When nothing later depends on the turn it is undone outright and leaves the history. When
   something does, the store changes nothing and returns a `Revert` (the adjusted edits, attributed to whoever
   asked, plus what it keeps); `vellum_core::ops::revert` applies it as a new transaction, so ⌘Z can take it back.
   ⌘Z itself never returns a Revert: it takes the actor's latest change back as far as it can, keeping later work,
   so pressing it again goes further back instead of toggling.
5. **A write that leaves a value as it was is not a change.** Edits carry whole nodes, so history sees which
   properties changed, not which an op named. If Bob sets a property to the value it already has, Alice's undo of
   her earlier change to it may still put the old value back. Invariant 3's property test uses the same rule.

### Numbers

Apple M5 Pro, release build, medians of 7, one page of 100 frames with 99 rectangles each (10,000 nodes). The batch
is ADR-004's: 200 leaves, 150 positions set and 50 moved into another frame, with another person's edit before it.

| | Time |
|---|---:|
| Plan the batch (`ops::plan`, Prompts 11–12) | 292 ms |
| Apply it (plan + the Loro store) | 304 ms |
| Undo it (Alice's, keeping Bob's work) | 14.5 ms |
| Redo it | 14.3 ms |

Undo fits in a frame at 10k, which is the bar this ADR set, but with little room: it copies the document once and
walks it to find each node (about edits × nodes), in `vellum-core`'s edit functions as much as in history. An id index
in the op layer would cut both. Planning is the real cost: `ops::plan` re-checks every reference after each op and
lists every id for each insert, so a 200-op transaction at 10k nodes takes about 0.3 s. That predates this prompt
and is recorded in `reports.md` for a person to schedule.

### Not done

- Not measured at 50k nodes, on M1 or Windows, or in WASM.
- Imports read the whole document back from Loro and compare it with the previous one (cost grows with the
  document). Reading only what Loro's events say changed is for Prompt 63, with sync.
- Concurrent edits that leave an invalid document (an instance whose component was removed elsewhere) are read back
  as they are; nothing reconciles them yet.
- Changes from another store are attributed to the newest change's origin (from Loro's commit message), not split
  per change, so "Undo this turn" does not reach a turn made on another device.

### Reference reading

- `refs/loro/crates/loro-internal/src/undo.rs`: the `UndoManager` keeps per-peer stacks of version spans and undoes
  by diffing the document between versions and transforming the result past later remote changes. Vellum keeps the
  idea of transforming an undo past later work, but works on its own edits and footprints, per actor and per turn,
  and never asks Loro for a diff; nothing was ported.
- `refs/loro/crates/loro-internal/src/handler/tree.rs` (`move_to`): the index of a move inside one parent is counted
  after the node leaves its place, as in Vellum's `Edit::Move`, so the store passes indexes through unchanged.
- `refs/loro/context/tree-events.md`, `loro-js-tree-diff.md` and `state/tree_state.rs`: tree events describe only the
  alive tree, and a concurrent move out of a deleted subtree keeps the moved node alive. Vellum reads the alive tree
  back whole after an import instead of applying events, and its test of a move concurrent with a delete expects
  exactly that.
