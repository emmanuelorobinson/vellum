# Document programs and script nodes

`vellum_core::doc_api::execute` accepts a ScriptRuntime, DocStore, an attributed request and an
ExecuteContext with injected IDs, time, Guard and services. One successful program makes one undo
entry. Exceptions, invalid Ops, limits and Guard rejections leave storage and leases untouched.
`validate_only` returns the prospective document and ChangeSet. Source is a strict function body;
use an explicit return for a result. See ADR-019 for policy limits and scene crop semantics.

The authoritative signatures are generated at
`packages/skills/src/generated/vellum-doc.d.ts` from `doc_api::REGISTRY`, using `@vellum/types-ts`.
`doc.apiVersion` is `1.0`. Generated node specifications allow omitted IDs and names; dimensions and
kind-specific required fields still follow the file schema. Methods return IDs; `get`/`query` return
copied stored nodes, including notes and context. Query matches exact type/name, optionally in one
page or subtree; unscoped query also visits component definitions. It does not expand instances.
`page` requires an existing unique name or ID. Local instances inherit definition dimensions when
omitted; imported instances need explicit dimensions. All mutations use existing validated Ops.

```sh
pnpm doc-api:gen
pnpm doc-api:check
cargo test -p vellum-script-quickjs --test doc_api
cargo test -p vellum-core --test script_nodes
node scripts/skia-build.mjs test -p vellum --test script_doc_api
node scripts/script-sandbox.mjs --offline
```

Supply `SceneServices` with the existing layout/text/font/spatial implementations and a `RenderRead`
binding to the shared renderer. Its library source must be an in-memory snapshot. All resources
must be loaded before execution: these callbacks cannot open files, fetch data or write exports.
`NoServices` refuses scene-dependent methods explicitly. Image fill accepts an already available
AssetId; icon accepts library:name. Permission-bearing I/O belongs outside this synchronous API.
The composition test `crates/vellum/tests/script_doc_api.rs` demonstrates the real adapter bindings.

## Script node header and output

The host loads the relative `.js` source and calls `script_nodes::evaluate` with the document, script
node, inherited Theme and an injected IdGen. Example script (TEST DATA):

```js
/**
 * @schema 1
 * @input count: number(min=1,max=20,step=1) = 3
 * @input caption: string(multiline=true) = "TEST DATA"
 * @input enabled: boolean = true
 * @input tint: color = "#123456"
 * @input mode: enum("compact","wide") = "compact"
 * @input target: ref(kind="page") = "page"
 */
return Array.from({length: doc.inputs.count}, () => ({
  type: "rect", width: 10, height: 20,
  fills: [{type: "solid", color: doc.inputs.tint}],
}));
```

Defaults/arguments are JSON literals. `enum(options=[...])` is equivalent to positional choices.
An omitted default requires an input value. Stored inputs are scalars or variable references; variable
colors arrive as hex strings. Refs use IDs, never paths. Script nodes return nodes and cannot mutate
the document or access file/render services. `VellumScriptNode<Inputs>` types this smaller surface.

Keep successes and ScriptDiagnostics outside DocStore. Feed fresh successful results to
`Resolver::set_script_outputs` or `SceneConfig::script_outputs`, keyed by document path and node ID.
After any source-file reload, input edit or variable/theme change, evaluate again; remove failed
results. Core also refuses mismatched cached inputs/variables/themes/references/source paths.
The original document serializes and undoes independently of these derived children.

To convert, put the current successful result's nodes in `ops::Inputs::script_outputs` and apply
`Op::ConvertScript` with the requesting person's Origin. This snapshots ordinary layers with fresh
IDs, common appearance and clip in one transaction. Missing/failed output must not be converted.
The UI renders diagnostics from `ScriptDiagnostic` (typed error, optional line/column and excerpt).

## Header fuzzing

```sh
cd crates/vellum-core
CARGO_NET_OFFLINE=true cargo +nightly fuzz run script_header -- -max_total_time=20 -max_len=4096
```

This needs installed cargo-fuzz and nightly. Ordinary `script_nodes` tests also run two 1,024-case
property checks, so parser coverage does not depend on a fuzz runner. CI currently runs those tests;
the live fuzz campaign is local. Preserve discovered failures as regressions before fixing them.
