# ScriptRuntime sandbox

The Prompt 25 adapter is `crates/adapters/script-quickjs`. It has no document mutation access.
The real document API, transaction staging, script nodes and generated declarations are Prompt 26.
See [ADR-005](../architecture/decisions/ADR-005-execute.md) for the supported language and limits.

## Calling the port

Construct `QuickJsRuntime::new(clock, seed)` with a monotonic millisecond clock callback owned by
that runtime. Native hosts can capture a `std::time::Instant`; WASM hosts supply a monotonic host
clock. The clock must progress for timeouts to work and must not be exposed as a script global.
The seed is a u32 and is reset for every run. No ambient randomness reaches script results.

Pass an `Rc<dyn ScriptApi>`. Its `functions` is the exact `doc` allowlist; every implementation must
provide `node_cost` and `call`. Compute cost from validated staged data, including all descendants
and copied nodes, before admitting a call. Stage changes and commit only after `run` succeeds.
Never block or perform I/O inside these synchronous callbacks; a JS interrupt cannot preempt Rust.

Scripts are strict function bodies. Use `return` to return finite JSON data. Unsupported or
oversized data refuses without truncation. No console is installed; logs are empty. Use
`Math.pow(x, y)` for powers: the exponent operators refuse because QuickJS's native operator calls
platform math. Transcendental Math functions use the same software implementation on all targets.

## Local checks

```sh
cargo test --offline -p vellum-script-quickjs
node scripts/script-sandbox.mjs --offline
node scripts/skia-build.mjs check --offline -p vellum-script-quickjs --target wasm32-unknown-emscripten
node scripts/spec-checksums.mjs check
```

The Node harness requires Node 24, the pinned Rust targets and emsdk 4.0.6 (`EMSDK`, or `~/emsdk`).
It builds only the adapter and fixture harness, with no renderer or browser requirement. Omit
`--offline` on a fresh machine so Cargo can fetch the locked dependencies. It checks the runtime
contract and escape/resource corpus in native and WASM, checks WASM imports, and compares exact
returned values and recorded calls. Outputs are `target/script-sandbox/{native,wasm}.txt`.
The ordinary script-test lane reports this test as skipped. To opt in:

```sh
VELLUM_LANES=wasm node --test scripts/__tests__/script-sandbox.test.mjs
```

`.github/workflows/script-sandbox.yml` runs the corpus on Linux, plus an emscripten type check.
The emscripten entry in `scripts/skia-build.mjs` configures bindgen's target sysroot and visibility.
If experimenting with those flags, Cargo may retain old generated bindings because the upstream
build script does not track these environment changes. Clean only the package's target output:

```sh
cargo clean -p rquickjs-sys --target wasm32-unknown-emscripten
```

The WASM fixture harness imports exactly `env.vellum_script_now_ms` (deadline only) and
`env.__rquickjs_host_now_us` (QuickJS initialization, fixed at zero in the harness). Script-visible
Date/performance/host imports are absent. The document API and derived output pipeline are documented in `doc-api.md` (Prompt 26).
Production daemon/editor tool wiring and C28 controls arrive in their later prompts.
