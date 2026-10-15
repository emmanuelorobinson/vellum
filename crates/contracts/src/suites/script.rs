//! `ScriptRuntime`.
//!
//! Kept to what every runtime does, the no-op one included. Prompt 25 adds the sandbox corpus (syntax errors,
//! loops, memory, escape attempts) for the real runtime.

use std::cell::Cell;
use std::rc::Rc;

use vellum_core::ports::{ScriptApi, ScriptError, ScriptLimits, ScriptRuntime, ScriptValue};

struct CountingApi(Cell<usize>);

impl ScriptApi for CountingApi {
    fn node_cost(&self, _: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
        Ok(0)
    }
    fn call(&self, _function: &str, _args: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
        self.0.set(self.0.get() + 1);
        Ok(ScriptValue::Null)
    }
}

const LIMITS: ScriptLimits = ScriptLimits {
    time_ms: 1000,
    max_ops: 100,
    max_nodes: 100,
    memory_mb: 16,
};

/// A script with nothing in it finishes without touching the API, the same source gives the same result, and
/// a large source does not bring the host down.
pub fn run(runtime: &dyn ScriptRuntime) {
    let api = Rc::new(CountingApi(Cell::new(0)));
    for source in ["", "   \n"] {
        runtime
            .run(source, api.clone(), LIMITS)
            .unwrap_or_else(|e| panic!("ScriptRuntime: {source:?} should finish: {e}"));
    }
    assert_eq!(
        api.0.get(),
        0,
        "ScriptRuntime: an empty script called the API"
    );

    let first = runtime.run("1 + 1", api.clone(), LIMITS);
    let second = runtime.run("1 + 1", api.clone(), LIMITS);
    assert_eq!(
        first, second,
        "ScriptRuntime: the same script gave two results (Invariant 8)"
    );

    let large = " ".repeat(1 << 20);
    let _ = runtime.run(&large, api, LIMITS);
}
