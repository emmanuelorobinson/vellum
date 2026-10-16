# ADR-005: synchronous QuickJS code mode and the document API boundary

- **Status:** Accepted for Prompt 25; the document API implementation and declaration generator are Prompt 26. ARCHITECTURE synchronized on the owner's instruction (2026-10-16).
- **Date:** 2026-10-15
- **Sources:** guide Invariant 8, §6.1 D5, Prompts 25–26; ARCHITECTURE §3.1 and §7.3; ADR-004's script spike.

## Decision

`vellum-script-quickjs` implements `ScriptRuntime` with rquickjs 0.14.0. Each run creates and drops its
own runtime and context, with no previous globals, objects, jobs or random state. Source is a strict
function body: use an explicit `return` for a value; otherwise the result is `Null`. No REPL state or
implicit final-expression rewriting. `ScriptOutput.logs` is empty: no console capability is installed.

Only the injected API's named functions appear on a frozen, null-prototype `doc` object. Each argument
and result is copied through bounded JSON data. The adapter never sees `DocStore` and never applies
an Op. API implementations must stage work, validate it through core Ops, and commit only after a
successful run. A failed run's staged changes must be discarded. Prompt 26 owns that transaction.

The port now takes `Rc<dyn ScriptApi>` instead of a borrowed API. QuickJS stores Rust callbacks;
owned handles avoid lifetime extension or unsafe borrowed-host trampolines. `ScriptApi::functions`
is an allowlist; mandatory `node_cost` computes the exact new-node count against staged data before
`call`, including descendants and copied subtrees. The runtime charges every API call, including
reads, and reserves the node cost before admitting it. The API is a trusted composition boundary;
scripts cannot supply or change the cost. Prompt 26 must test these costs against its real Op plans.

## Capabilities and limits

- Intrinsics: base objects, evaluation for the host compiler, JSON, Map/Set, and internal Promise
  machinery. No filesystem, network, OS modules, loader, timers, Date, performance, workers, Proxy,
  typed arrays, RegExp compiler, weak references or host handles. No rquickjs loader/futures features.
- JavaScript eval and all reachable function constructors refuse with `Forbidden`. Built-in objects
  and prototypes are frozen. Literal async functions and dynamic imports are caught by a promise
  hook, even when their result is discarded or their exception caught. Jobs are never executed.
  Promise machinery must be initialized internally: omitting it while inspecting async function
  prototypes caused a reproducible QuickJS teardown assertion. The global Promise is unavailable.
- A caller supplies a monotonic millisecond clock and a u32 seed. Only the interrupt handler and
  host-call deadline checks see the clock. Math.random uses a fresh wrapping u32 LCG per run
  (`1664525 * state + 1013904223`, divided by `2^32`). Seed zero is valid. This is not cryptographic.
- Time covers source preflight, sandbox setup, user-source compilation, execution, conversion and
  diagnostics. The interrupt handler stops busy loops. Deadline checks also bracket host
  calls and their node-cost planning. Host calls must be synchronous and bounded: JavaScript
  interruption cannot preempt a blocking Rust function. Hosts must not perform I/O in these calls.
- The runtime uses a bounded allocator wrapping rquickjs's RustAllocator, charging live allocations
  including alignment and conservative bookkeeping. Its failure flag survives caught OOM errors.
  No secondary engine limit is used: it could throw before that flag is set. This also works if a
  future dependency enables rust-alloc through Cargo feature unification. Unsafe code is confined
  to the allocator delegation; it never dereferences or changes allocation pointers itself.
- Memory, time, API-call, node and host-API failures are sticky; catching a JavaScript exception
  cannot restore host access or make the run succeed. Zero memory/time budgets refuse immediately;
  zero op/node budgets still permit pure computation/read calls respectively.
- Cross-boundary data is finite JSON, with undefined mapped to null, maximum depth 32 and 65,536
  values per conversion. Source lexical nesting is capped at 128. Accessors, cycles, exotic objects, symbol/function/BigInt values, prototype
  keys and sparse/extended arrays refuse. Nonenumerable array indices keep their values. Source is
  capped at one quarter of runtime memory; cumulative serialized boundary traffic at the smaller
  of one quarter of runtime memory and 1 MiB. These are explicit refusal limits, never truncation.
- Diagnostics copy only primitive values from own property descriptors, never invoking error getters
  or arbitrary string conversion. Error stacks retain the evaluation filename; the strict wrapper
  adds one source line. Prompt 26 owns structured source excerpts and line/column presentation.

ARCHITECTURE's caller policies remain 5 s / 2,000 API calls / 64 MiB for execute, and 2 s / 1,000
created nodes for script nodes. This adapter accepts explicit `ScriptLimits`; Prompt 26 supplies
those policies and its execute node ceiling. There are no document mutations or UI in Prompt 25.

## Numeric determinism

The first cross-target transcendental fixture found `Math.tan(1e20)` differed by one bit between
Darwin's libm and WASM's libm. Returning approximate values or rounding only at the boundary would
still allow different branches and operations. Math transcendental functions, sqrt, pow and hypot
therefore use pinned libm 0.2.16 with `force-soft-floats`, overriding architecture-specific feature
unification. JS coercion, missing-argument and pow/hypot edge cases are preserved.

The exponent operators `**` and `**=` bypass Math and call QuickJS's platform pow, so they refuse
with `Forbidden` directing the script to `Math.pow`. A lexical guard skips quoted strings, comments
and template text while inspecting template substitutions, including all JS line terminators and
HTML line comments. RegExp literals remain unsupported, which keeps slash unambiguous. This is a
language restriction beyond ARCHITECTURE; remove it only when QuickJS's operator has portable math.

Successful deterministic fixtures compare returned values and recorded API calls byte for byte in
native and wasm32-unknown-unknown. They cover random layout, JSON, loops, arrays and every replaced
Math operation. Failed runs never provide a successful output; timing/allocator cutoff positions
are not promised identical across machines. Linux/Windows native runs remain for CI/other hosts.

## Generated declarations (Prompt 26)

Keep one versioned descriptor registry for the real document API: names, argument/result shapes,
node-cost planning and implementation bindings. Generate `vellum-doc.d.ts` from that registry and
`@vellum/types-ts`; do not maintain a second handwritten schema or derive names from arbitrary JS.
Expose the API version from the same registry. Add a regeneration/diff check and compile examples
against the generated declarations. Give these declarations to agent skills alongside the tool
registry; runtime authorization, validation and limits remain independent of types. No declaration
is generated for the fake fixture API in this prompt.

## Reference study

rquickjs runtime/base/raw and context/builder provide bounded allocation, stack limits, interruption
and selective intrinsics; its WASM harness demonstrates a minimal host clock import. Vellum keeps
these seams, adds sticky failures and omits the job queue and all host capabilities beyond doc.
Codex code-mode delegates cells and callbacks through an isolated runtime; OpenCode describes an
allowlisted interpreter, copied boundary values, budgets and generated model-facing signatures.
Vellum keeps typed code-mode discovery and copying, uses synchronous QuickJS on native/WASM, and
stages one transaction rather than streaming tool effects. Sibling searches found no reusable
sandbox. No reference implementation was ported and no source from pen.dev was used.

## Validation and revisit

`cargo test -p vellum-script-quickjs` runs the contract, escape corpus, typed limits and boundary
regressions. `node scripts/script-sandbox.mjs` runs the same corpus natively and in WASM with a
real injected monotonic clock, verifies the two-import WASM boundary, and compares transcripts.
The separate CI workflow also checks the adapter for emscripten. This session executed macOS arm64
and WASM; configured GitHub jobs have not been run remotely. See reports.md for exact commands.

Revisit on an escape, nondeterministic successful fixture, unacceptable cold-run cost, a QuickJS
security update, or a need for synchronous capabilities outside doc. Supporting async or I/O
requires a new decision, cancellation model and permission boundary; adding globals is insufficient.
