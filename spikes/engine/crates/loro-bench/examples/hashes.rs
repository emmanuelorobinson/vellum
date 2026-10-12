//! Prints the same hashes as the WASM module's `lb_*_hash` exports, for a native/WASM comparison.
use spike_loro::{fnv1a, from_document, load, save, to_json};
fn main() {
    let scene = spike_scene::generate(1_000);
    let list = spike_scene::DisplayList::build(&scene);
    let bytes: Vec<u8> = list.ops.iter().flat_map(|v| v.to_le_bytes()).collect();
    let snapshot = save(&from_document(&scene));
    let json = to_json(&load(&snapshot).unwrap());
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, &json).unwrap();
    }
    println!(
        "scene {} snapshot {} ({} bytes) json {}",
        fnv1a(&bytes),
        fnv1a(&snapshot),
        snapshot.len(),
        fnv1a(json.as_bytes())
    );
}
