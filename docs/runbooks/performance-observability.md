# Performance and observability

Use Node 24, pnpm 12 and the pinned Rust/emsdk toolchains. Set `TURBO_TELEMETRY_DISABLED=1` and
`NEXT_TELEMETRY_DISABLED=1`. The workload requires release builds and rejects debug measurements.
See ADR-031 for the added idle RSS guardrail and interpretation of the WASM budget.

```sh
pnpm performance:native
pnpm wasm:build
pnpm performance:browser --software
# Hardware GPU lane, with a configured Chrome and hardware WebGL2:
pnpm performance:browser
node scripts/performance-budget.mjs target/performance/native.json /path/to/same-machine-base.json
```

Native measurements write `target/performance/native.json` and a disposable TEST DATA document:
an exactly 2 MiB file, 5,000 drawable children, including 500 text nodes in bundled Inter, plus a root.
JSON and directory decoding, Loro storage, Taffy, Parley, Rstar, native Skia and the native Basic reference
path participate. Open includes a real disk read with a warm OS cache, decode, initial store and first
pixels. Pan/zoom deliberately rebuilds the complete engine pipeline. Drag updates a node through an Op;
batch inserts 200 nodes through an agent-origin transaction, including planning/validation, Loro commit,
snapshot and paint. The harness does not measure OS event dispatch or installed-host startup.
The directory open includes decode and paint; its on-disk serialized size differs from the 2 MiB JSON.

The idle sampler launches the actual daemon in a private temporary directory, polls authenticated status,
then samples the process's resident memory twenty times over about two seconds. It uses `ps` on macOS/Linux
and WorkingSet64 on Windows. It terminates its own process and deletes only its own state, including on
failure. This measures a ready empty daemon, not memory after a working session. The cap is under 128 MiB.

The browser harness measures display-list upload, preparation and completed draw on all shipped renderer
paths: Skia WebGL2, Skia WASM CPU and Canvas2D. GPU completion uses `gl.finish`; Canvas2D forces a pixel
read. CPU measurements upload/prepare/rasterize without an extra GPU draw. Fixture download and core
Ops/layout are outside these draw-only timings. The 200-node draw is not claimed as agent-batch latency;
the native gate measures the attributed transaction pipeline. Browser document-open/input-to-frame
acceptance still needs P55's shared document ABI. Software WebGL2 is integration evidence, not M1 or
mid-range Windows hardware acceptance. Reports include all p50/p95/max values and sample counts.

`scripts/performance-budget.mjs` fails for missing adapters/measurements, nonfinite or malformed samples,
absolute budget breaches and p95 regressions above 10%. Renderer native CI enforces absolute native and RSS
budgets on all six platform/architecture lanes. Hardware browser lanes enforce renderer and WASM budgets;
software integration records the same measurements. `performance.yml` additionally builds base and candidate
on one runner with the same measurement harness and checks both absolute and regression gates. A new
performance task never uses Turborepo caching. No baseline update or golden recording happens automatically.

Repeated shape resources share one prepared native path. Basic plans transfer one SVG geometry per path
and numeric references, then reuse Path2D within a frame. Legacy inline plans remain readable, and invalid
references fail before the previous frame is replaced. This bounds repeated transfer/parsing work; it does
not make the 5,000-node Canvas2D path meet the frame budget. See the recorded timings before assuming it does.

§12 concerns the local document/frame hot path. Service adapters (model/installed engine, sync, accounts,
image providers, imports/exports, secrets, permissions and telemetry) retain their contract/integration lanes;
this benchmark does not turn mock network timings into frame-rate evidence. `render-vello` remains an
unimplemented optional shell and is not advertised as a measured renderer. A new functional hot-path adapter
must join the required adapter list and workloads, rather than inheriting the default adapter's results.

Golden flake controls: all resources are local fixture bytes; await the module, complete synchronous paint,
pixel readback and browser font readiness. Every native/browser fixture is painted twice with identical
inputs; differing repeated pixels fail immediately. The renderer contract, which disposes its renderer,
runs after measurement. No sleeps, random fixtures, test retries, recording mode or tolerance changes hide
pixel differences. The performance sample warm-up is separate from golden acceptance.

## Telemetry

Editor Settings already states: off by default; version, OS, renderer failures and anonymous feature counts;
never files, prompts, keys or repo paths. Defaults remain off for the editor, CLI, daemon and cloud service.
Desktop and VS Code native hosts read `VELLUM_TELEMETRY_ORIGIN` as the configured HTTPS relay origin;
no origin means no outbound request, even if Settings is enabled. The webview receives no URL or token.
Consent persists with the existing editor preferences. VS Code's global telemetry disable also aborts
pending reporting. Closing a window or opting out aborts its pending requests. Reports from disabled periods
are dropped, never replayed when consent changes. No network request is made during the default path.

```sh
vellum telemetry status --json
vellum telemetry on --origin https://configured-api.example
vellum telemetry off
```

The CLI prints the disclosure with status/on/off. Only `{enabled, origin}` is persisted beside the private
run directory in `telemetry.json`; this is separate from documents and sessions. Missing or corrupt settings
fail closed. CLI/daemon start reporting is bounded to two seconds and emits at most one fixed start event
per process. Changing consent affects future process starts; there is no periodic daemon telemetry task.
No collector key, credential or account is required locally. Only use an origin the person configured.

The cloud API operator configures `VELLUM_TELEMETRY=true` plus `VELLUM_POSTHOG_ORIGIN` and
`VELLUM_POSTHOG_TOKEN`, and/or `VELLUM_SENTRY_DSN`, in server secret configuration. Default cloud forwarding stays off; explicit opt-in without a complete collector configuration fails
at startup with a payload-free error. `/v1/telemetry` accepts at most 1 KiB and 120 validated reports per service
minute without storing IPs. The shared sender bounds four in-flight reports with a two-second timeout.
Excess events drop; no queue, retry or delivery guarantee. Provision an ingress-wide limit when deploying
multiple instances. Do not enable request/body logging for this route at the proxy.

The exact client JSON is `{event, version, os, count: 1}`. Event names are allowlisted in
`packages/telemetry/src/index.ts`: editor_opened, export_completed, renderer_failed, cli_started,
daemon_started, cloud_request, cloud_failed. Only editor_opened, process starts and cloud_failed are
currently instrumented. Other names are available for explicit future call sites, with no automatic SDK
capture. Sentry receives only fixed error-event names/version/OS; PostHog receives the safe counts with
person profiles and GeoIP processing disabled. Per-event random identifiers are added server-side for
protocol requirements. No persisted analytics identity, user email, cookie, request, raw error, stack,
breadcrumb, file bytes, path, document title, prompts or provider key is forwarded. Cloud providers still
see the relay's network connection; configure their retention and access policies before enabling them.

The tests inject fake collectors and canaries into payloads, headers, errors and nested properties. Real
PostHog/Sentry projects, external delivery/retention, installed-host consent acceptance and M1/Windows
hardware acceptance need their deployment/platform lanes. Current native performance failures remain
release blockers; see `reports.md` and the JSON artifacts. No release acceptance is implied by having gates.
