//! The engine's C ABI (ADR-033), Emscripten only. Same ownership rules as the renderer's: JS writes only into
//! buffers this module allocated (`vellum_alloc`), every input buffer is consumed, and every returned buffer is
//! JS's to read and `vellum_free`. Failures return 0 and leave a message for `vellum_error`.
use std::cell::RefCell;
use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::engine::{Engine, Options};
use crate::renderer::{give_buffer, set_error, take_buffer};

thread_local! {
    static ENGINES: RefCell<(u32, BTreeMap<u32, Engine>)> = RefCell::new((0, BTreeMap::new()));
}

fn json_in(p: usize) -> Result<Value, String> {
    serde_json::from_slice(&take_buffer(p)?).map_err(|e| format!("Invalid request: {e}"))
}

fn out(result: Result<Vec<u8>, String>) -> usize {
    match result {
        Ok(bytes) => give_buffer(bytes),
        Err(e) => {
            set_error(e);
            0
        }
    }
}

fn with<T>(handle: u32, f: impl FnOnce(&mut Engine) -> Result<T, String>) -> Result<T, String> {
    ENGINES.with_borrow_mut(|(_, engines)| {
        f(engines
            .get_mut(&handle)
            .ok_or_else(|| "Unknown engine".to_owned())?)
    })
}

/// Opens an engine. `options` is `{path, peer, seed, readOnly}` (peer and seed as decimal strings, since a
/// JS number cannot hold 64 bits); `file` is the `.vellum` bytes, or the daemon's genesis document when
/// `updates` (non-zero) brings the replica up to date. Returns a handle, or 0.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_open(options: usize, file: usize, updates: usize) -> u32 {
    let result = (|| {
        let o = json_in(options)?;
        let bytes = take_buffer(file)?;
        let number = |key: &str| {
            o[key]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| format!("{key} is a decimal string"))
        };
        let options = Options {
            path: o["path"].as_str().ok_or("path is missing")?.to_owned(),
            peer: number("peer")?,
            seed: number("seed")?,
            read_only: o["readOnly"].as_bool().unwrap_or(false),
        };
        let engine = if updates == 0 {
            Engine::open(&bytes, options)?
        } else {
            Engine::replica(&bytes, &take_buffer(updates)?, options)?
        };
        Ok::<_, String>(ENGINES.with_borrow_mut(|(next, engines)| {
            *next += 1;
            engines.insert(*next, engine);
            *next
        }))
    })();
    result.unwrap_or_else(|e| {
        set_error(e);
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_close(handle: u32) {
    ENGINES.with_borrow_mut(|(_, engines)| {
        engines.remove(&handle);
    });
}

/// One facade call: a JSON request in, its JSON answer out.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_call(handle: u32, request: usize) -> usize {
    out(json_in(request).and_then(|r| {
        with(handle, |e| e.call(&r)).map(|v| serde_json::to_vec(&v).unwrap_or_default())
    }))
}

/// Registers a font face; answers the JSON list of handles frames will name it by.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_font(handle: u32, bytes: usize) -> usize {
    out(take_buffer(bytes).and_then(|b| {
        with(handle, |e| e.register_font(b)).map(|h| {
            serde_json::to_vec(&h.iter().map(ToString::to_string).collect::<Vec<_>>())
                .unwrap_or_default()
        })
    }))
}

/// The canonical `.vellum` bytes of the document now.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_encode(handle: u32) -> usize {
    out(with(handle, |e| e.encode()))
}

#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_cursor(handle: u32) -> usize {
    out(with(handle, |e| Ok(e.cursor())))
}

/// What a peer at `cursor` lacks (the person's edits, for the daemon).
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_updates(handle: u32, cursor: usize) -> usize {
    out(take_buffer(cursor).and_then(|c| with(handle, |e| Ok(e.updates_since(&c)))))
}

/// Imports the daemon's updates: 2 when something changed, 1 when nothing did, 0 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_import(handle: u32, updates: usize) -> u32 {
    match take_buffer(updates).and_then(|b| with(handle, |e| e.import(&b))) {
        Ok(true) => 2,
        Ok(false) => 1,
        Err(e) => {
            set_error(e);
            0
        }
    }
}

/// A viewport's display list: a little-endian u32 length, a JSON header (`{"fonts": [{handle, index}]}`, the
/// faces it draws with), then the display list bytes.
#[unsafe(no_mangle)]
pub extern "C" fn vellum_engine_frame(handle: u32, request: usize) -> usize {
    out(json_in(request).and_then(|r| {
        with(handle, |e| e.frame(&r)).map(|frame| {
            let header = serde_json::to_vec(&json!({
                "fonts": frame.fonts.iter().map(|(h, d)| json!({"handle": h.0.to_string(), "index": d.index}))
                    .collect::<Vec<_>>(),
            }))
            .unwrap_or_default();
            let mut bytes = (header.len() as u32).to_le_bytes().to_vec();
            bytes.extend(header);
            bytes.extend(frame.list);
            bytes
        })
    }))
}
