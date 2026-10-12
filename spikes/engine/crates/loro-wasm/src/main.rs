//! The Loro measurements in WASM (guide Prompt 5): each step is one export, and Node times the calls,
//! so the module needs no clock. Mirrors `loro-bench`; see that binary for what each step means.
//! Built for wasm32-unknown-emscripten with `scripts/build-wasm.sh`.

use std::cell::RefCell;

use loro::{ExportMode, LoroDoc, VersionVector};
use spike_loro::{BatchEdit, apply_edits, from_document, load, node_count, pick_batch, save};
use spike_scene::{Document, generate};

#[derive(Default)]
struct State {
    scene: Option<Document>,
    doc: Option<LoroDoc>,
    snapshot: Vec<u8>,
    loaded: Option<LoroDoc>,
    remote: Option<LoroDoc>,
    edits: Vec<BatchEdit>,
    vv: Option<VersionVector>,
    update: Vec<u8>,
}

thread_local! {
    static S: RefCell<State> = RefCell::new(State::default());
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_generate(nodes: usize) -> usize {
    S.with_borrow_mut(|s| {
        let scene = generate(nodes);
        let n = scene.node_count();
        s.scene = Some(scene);
        n
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_build() -> usize {
    S.with_borrow_mut(|s| {
        let doc = from_document(s.scene.as_ref().expect("generate first"));
        let n = node_count(&doc);
        s.doc = Some(doc);
        n
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_save() -> usize {
    S.with_borrow_mut(|s| {
        s.snapshot = save(s.doc.as_ref().expect("build first"));
        s.snapshot.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_load() -> usize {
    S.with_borrow_mut(|s| {
        s.doc = None;
        let doc = load(&s.snapshot).expect("load");
        s.loaded = Some(doc);
        s.snapshot.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_full_read() -> usize {
    S.with_borrow(
        |s| match s.loaded.as_ref().expect("load first").get_deep_value() {
            loro::LoroValue::Map(m) => m.len(),
            _ => 0,
        },
    )
}

/// FNV-1a of the snapshot bytes and of the state's JSON, to compare with the native build.
#[unsafe(no_mangle)]
pub extern "C" fn lb_snapshot_hash() -> u64 {
    S.with_borrow(|s| spike_loro::fnv1a(&s.snapshot))
}

thread_local! {
    static JSON: RefCell<String> = const { RefCell::new(String::new()) };
}

/// The state's JSON, for debugging a native/WASM difference: returns a pointer, `lb_json_len` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn lb_json_ptr() -> *const u8 {
    let json = S.with_borrow(|s| spike_loro::to_json(s.loaded.as_ref().expect("load first")));
    JSON.with_borrow_mut(|j| *j = json);
    JSON.with_borrow(|j| j.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_json_len() -> usize {
    JSON.with_borrow(|j| j.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_json_hash() -> u64 {
    S.with_borrow(|s| {
        spike_loro::fnv1a(spike_loro::to_json(s.loaded.as_ref().expect("load first")).as_bytes())
    })
}

/// FNV-1a of the generated scene's display list, to see whether the input differs between targets.
#[unsafe(no_mangle)]
pub extern "C" fn lb_scene_hash() -> u64 {
    S.with_borrow(|s| {
        let list = spike_scene::DisplayList::build(s.scene.as_ref().expect("generate first"));
        let bytes: Vec<u8> = list.ops.iter().flat_map(|v| v.to_le_bytes()).collect();
        spike_loro::fnv1a(&bytes)
    })
}

/// Prepares a decoded remote peer and picks a batch (untimed setup).
#[unsafe(no_mangle)]
pub extern "C" fn lb_prepare_batch(seed: u64) -> usize {
    S.with_borrow_mut(|s| {
        let remote = load(&s.snapshot).expect("load");
        remote.set_peer_id(2).expect("peer");
        let _ = remote.get_deep_value();
        s.remote = Some(remote);
        let loaded = s.loaded.as_ref().expect("load first");
        s.vv = Some(loaded.oplog_vv());
        s.edits = pick_batch(loaded, seed);
        s.edits.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_apply_batch() -> usize {
    S.with_borrow_mut(|s| {
        apply_edits(s.loaded.as_ref().expect("load first"), &s.edits);
        s.edits.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_export_update() -> usize {
    S.with_borrow_mut(|s| {
        let vv = s.vv.take().expect("prepare first");
        s.update = s
            .loaded
            .as_ref()
            .expect("load first")
            .export(ExportMode::updates(&vv))
            .expect("update");
        s.update.len()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lb_remote_import() -> usize {
    S.with_borrow_mut(|s| {
        s.remote
            .as_ref()
            .expect("prepare first")
            .import(&s.update)
            .expect("import");
        s.update.len()
    })
}

/// 1 when the remote peer equals the local document.
#[unsafe(no_mangle)]
pub extern "C" fn lb_converged() -> u32 {
    S.with_borrow(|s| {
        u32::from(
            s.remote.as_ref().map(LoroDoc::get_deep_value)
                == s.loaded.as_ref().map(LoroDoc::get_deep_value),
        )
    })
}

fn main() {}
