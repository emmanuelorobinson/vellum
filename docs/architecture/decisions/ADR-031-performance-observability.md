# ADR-031 — Performance gates and minimal opt-in observability

Status: accepted implementation decision; release performance acceptance remains open.

Prompt 71 makes ARCHITECTURE §12 executable without changing the specification. Timing uses release
builds, three warm-up paints and twenty or more measured samples, with nearest-rank p95. The independent
absolute gates are 500 ms open, 1000/60 ms pan/zoom, 16 ms drag and 100 ms attributed 200-node batch.
The native benchmark includes the full resolve/layout/display-list/raster pipeline; it is a conservative
engine measurement, not installed-host input latency. Browser renderer measurements start at an already
built display list. Neither substitutes for the other's work. Cold installed-host startup requires P55.

Use a 2.4 MiB gzip ceiling for the single core/Skia/Parley/QuickJS module: §12's approximately 2 MiB plus
approximately 0.4 MiB for QuickJS. Fonts remain side resources. Add an explicit **under 128 MiB idle RSS**
release gate for a newly launched, ready, empty daemon. The specification supplies no numeric RSS budget;
this is a new release guardrail, not a number inferred from it. Keep measurement and gate failures visible.

Regressions above 10% fail independently of absolute budgets. CI measures base and candidate sequentially
on the same runner with the same harness and toolchain. It overlays only test harness files in an isolated
base checkout under target. Product implementations, locks and golden references stay those of each revision.
No committed cross-machine timing baseline, automatic baseline update, tolerance widening or retry-to-green.
A baseline which exceeded a budget can be replaced by a candidate which fixes it; malformed baselines fail.

Repeated shape resources are prepared once per display list. The fallback transfers an indexed geometry
dictionary and reuses each Path2D within the frame; old inline geometry remains readable. This derived
transport is rebuilt with its matching WASM module and is never a persisted document format.

Local telemetry has two independent requirements: explicit person consent and a configured HTTPS relay.
The relay never requires an account. Rust sends fixed process-start events; native editor hosts send their
own fixed editor-open event after consent. Webviews can change a boolean only, through their existing
sender-checked bridge. They gain no network policy or credential access. VS Code's telemetry opt-out wins.
Cloud operators must separately opt in and configure PostHog and/or Sentry. They receive reconstructed
fixed event names, version, OS and count=1. Arbitrary errors, stacks, requests, URLs, paths, document data,
prompts, identity and keys have no payload fields. Cloud credentials stay in server configuration; clients
hold none. Event UUIDs are per report, never persistent identity. No automatic capture/replay/profiling SDK.
Transport failures are silent best-effort drops, with bounded concurrency/time and no disk queue or retry.

Reference study: Penpot's visual-regression guide requires waiting for its first rendered frame and mocked
assets. Its playground separates fixture rendering from the host. Vellum retains readiness/asset waits and
an isolated shared-renderer harness, adds a second identical paint to detect nondeterminism, and keeps its
own unchanged native golden files/tolerances. No source, text, helpers or assets were ported. Sibling Outpost
uses fixed metric/attribute names and no raw logs; Vellum adopts that principle with its own event schema.
