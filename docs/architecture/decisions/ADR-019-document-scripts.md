# ADR-019: atomic document programs and derived script nodes

- **Status:** Accepted for Prompt 26; human review and ARCHITECTURE synchronization pending. ARCHITECTURE synchronized on the owner's instruction (2026-10-16).
- **Date:** 2026-10-15
- **Sources:** PRODUCT script nodes/checked agent edits; ARCHITECTURE §4.1, §7.3; DESIGN C28;
  guide Prompt 26 and Invariants 1–4, 6, 8, 11; ADR-005.

## Decision

The pure core owns `doc_api::execute`, the method registry and script-node semantics. QuickJS only
copies JSON and invokes the `ScriptApi` port. Its new constants are data properties, recursively
frozen with `doc`. `apiVersion` and declarations come from one registry; `doc-api:check` compares the
artifact and the skills package compiles examples with declaration checking enabled.

Every mutating call first plans its real Op against a staged document, caches that plan and reserves
its materialized node cost before admitting it. The call installs the plan's document in the stage,
never in DocStore. Costs include inserted descendants, instance override/slot content and component
definition trees, including replacement roots; reads cost zero nodes. ID allocation can consume
injected IDs on refusal or dry run; IDs are never recycled. API calls are also limited independently.
The complete successful stage is diffed back into explicit-ID Ops and planned once more against the
original store snapshot, then committed once. Both staged and live Guard state must allow it. A dry
run does the same validation and returns its prospective document/ChangeSet without a commit.
Claim/release stage session leases and roll back with the program; successful own-frame activity
refreshes leases. Lease-only programs do not create document undo entries.

Execute clamps caller budgets to 5 seconds, 2,000 API calls, 2,000 materialized nodes and 64 MiB;
callers can lower them. The existing Guard limits still apply to agents. Its defaults can therefore
refuse an agent program before the execute ceiling. Each API mutation must independently leave
valid references; forward references between separate doc calls are refused. This is stricter than
the raw Op transaction's end-only reference check. A no-net-change program makes no undo entry.

`Services` and `RenderRead` accept only bounded synchronous in-memory work. `SceneServices` resolves
the staged document, runs injected layout/text/spatial ports and builds the existing display list.
Child reads retain page ancestry, transforms and inherited themes. Screenshots/exports crop the
shared page scene to the selected target's world box, including overlapping paint. They return MIME,
base64 and dimensions; there is no file-writing capability. Rendering is capped at 8,192 pixels per
axis, 16,777,216 pixels total and 700 KiB encoded-input bytes; the sandbox's cumulative JSON budget
can refuse smaller results too. Libraries/fonts/images must already be available to the injected
services, without file/network access in a call. `fillImage` uses an existing AssetId. `icon` inserts
an existing library:name reference (weight 400 by default); it does not fetch or synthesize artwork.

## Script nodes

Leading comments declare schema 1 or 1.0 and up to 128 unique inputs in a 64 KiB header. Defaults and
arguments use JSON values. Number min/max are constraints; step is a positive control increment.
String supports multiline, enum has explicit unique choices, color uses the existing Color parser,
and ref validates an existing node/page (or an explicitly requested component/variable). Missing
schema means 1; missing defaults require supplied inputs. Unknown declarations/types/arguments,
invalid defaults, unknown supplied inputs and unsupported schemas refuse explicitly.

A node program receives only frozen `doc.inputs` and `doc.apiVersion`, and explicitly returns an array
of schema nodes. It has no document-writing API. Input variable resolution uses the caller's inherited
theme. Execution is limited to 2 seconds/64 MiB; output is capped at 1,000 nodes, checked through Ops
in an isolated staging page and given ephemeral IDs. Direct recursive script output is refused.
The host supplies source text after resolving its relative path outside this pure module.

Successful outputs are separate from Document and DocStore. The resolver projects them into ordinary
frames for the shared layout/display path, retaining clipping and appearance. It refuses stale input,
variable/theme/reference/source-path caches explicitly. Source-file changes require the host to
replace its output map; failed evaluations must remove old successful entries. `ConvertScript` takes
successful output through `ops::Inputs`, remints IDs and replaces the script with a frame in one
attributed, undoable transaction. No document schema or stored derived children were added.

QuickJS supplies source positions for API, syntax and runtime exceptions. An intrinsic captured
Error stack getter reads engine traces without invoking script-supplied getters. The strict wrapper's
one-line offset is removed. Diagnostics retain typed causes, optional one-based line/column and a
bounded three-line excerpt centered near the failing column. Positions unavailable for early host,
allocator or deadline failures stay absent; they are not invented.

## Reference study

Penpot's execute tool forwards explicit-return programs into a persistent host plugin context.
Codex isolates code-mode cells; OpenCode describes allowlisted tools, copied values and generated
signatures with independent budgets. Vellum keeps explicit returns, copying and registry discovery,
uses fresh synchronous QuickJS realms, and stages one Ops commit instead of streaming tool effects.
Sibling searches found no reusable document API/parser. No reference code was ported.

## Validation and limits

The adapter integration tests cover writes/read-after-write, atomic failures, dry runs, node costs,
slots/components, leases, diagnostics, input binding, derived rendering and invalidation. A native
composition test runs QuickJS through Taffy and the shared Skia renderer for PNG/SVG/PDF. The existing
native/WASM harness now compares actual document-program and script-node transcripts as well as the
escape corpus. Header property tests and a cargo-fuzz target exercise malformed Unicode and grammar.

Only macOS arm64 and Node WASM ran here. The existing Linux sandbox CI runs adapter integration tests;
remote execution, production editor/daemon wiring and a prolonged fuzz campaign remain unverified.
Prompt 30 owns the MCP execute tool and Prompt 52 owns C28 chrome. Synchronous host work cannot be
preempted by the JS interrupt; injected services must honor the bounded no-I/O contract. See reports.md
for exact commands and known unrelated repository failures.
