//! rquickjs with a toy document API (guide Prompt 5). Native, no lane. The wasm32 run is in
//! `web/script.wasm.test.mjs` (lane `wasm`).

use spike_script::{Limits, Op, SCRIPT_500, ScriptError, run};

#[test]
fn the_500_op_script_records_500_ops_deterministically() {
    let ops = run(SCRIPT_500, &Limits::default()).expect("script runs");
    assert_eq!(ops.len(), 500);
    assert!(matches!(ops[0], Op::Create { id: 1, .. }));
    let creates = ops
        .iter()
        .filter(|op| matches!(op, Op::Create { .. }))
        .count();
    assert_eq!(creates, 100);
    assert_eq!(ops, run(SCRIPT_500, &Limits::default()).unwrap());
}

#[test]
fn an_endless_loop_is_interrupted_by_the_budget() {
    let err = run("while (true) {}", &Limits::default()).unwrap_err();
    assert_eq!(err, ScriptError::Interrupted);
}

#[test]
fn ops_made_before_an_interrupt_are_not_returned() {
    let err = run("doc.create('rect', 0); while (true) {}", &Limits::default()).unwrap_err();
    assert_eq!(err, ScriptError::Interrupted);
}

#[test]
fn allocating_past_the_memory_limit_fails() {
    let limits = Limits {
        memory_bytes: 8 * 1024 * 1024,
        ..Limits::default()
    };
    let err = run(
        "const a = []; for (;;) a.push(new Array(100000).fill(1));",
        &limits,
    )
    .unwrap_err();
    assert_eq!(err, ScriptError::OutOfMemory);
}

#[test]
fn deep_recursion_is_a_catchable_stack_error() {
    let err = run(
        "function f(n) { return f(n + 1) + 1; } f(0);",
        &Limits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&err, ScriptError::Exception(m) if m.contains("stack")),
        "{err:?}"
    );
}

#[test]
fn the_sandbox_has_no_host_access() {
    let probe = "for (const name of ['require', 'process', 'std', 'os', 'fetch', 'XMLHttpRequest', \
                 'WebAssembly', 'setTimeout', 'importScripts']) { \
                   if (typeof globalThis[name] !== 'undefined') throw new Error(name + ' is reachable'); \
                 }";
    run(probe, &Limits::default()).expect("no host globals");
}

#[test]
fn bad_arguments_to_the_doc_api_throw() {
    let err = run("doc.set(999, 'x', 1)", &Limits::default()).unwrap_err();
    assert!(matches!(err, ScriptError::Exception(_)), "{err:?}");
}

#[test]
fn a_kept_engine_runs_the_same_script_again() {
    let engine = spike_script::Engine::new(&Limits::default()).expect("engine");
    for _ in 0..3 {
        assert_eq!(engine.run(SCRIPT_500).expect("runs").len(), 500);
    }
}
