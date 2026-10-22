# Security checks and release acceptance

Prompt 69. Read [the threat model](../architecture/threat-model.md) before changing trust boundaries.
These commands never publish, sign with a real identity, install an update or change a user's keys.
Keep developer telemetry disabled. Use Node 24, pnpm 12 and the pinned Rust toolchain.
Install with `pnpm install --frozen-lockfile` and populate the Cargo source cache with
`cargo fetch --locked` before generating the offline source SBOM.

## Commands and evidence

```sh
pnpm security:check
pnpm security:sbom
pnpm security:fuzz
pnpm security:canary
```

`security:check` runs Rust boundary/escape/token tests, the fixed op corpus, desktop/extension tests,
dependency rules, the complete cargo-deny advisory policy, pnpm audit at low severity and the specification
checksum. Every command runs even when an earlier check fails. A nonzero status, missing executable,
signal, timeout or malformed SBOM fails the gate. No advisory exception or severity filter is introduced.
No skipped tagged test counts as an acceptance pass.

Logs and per-command status are in `target/security/`. Check `check.json`, `fuzz.json` and `sbom.json`
before interpreting artifacts; an old artifact from an earlier run is not current evidence. Reports are
source/build inputs for review, not a signed binary attestation. The workflow keeps artifacts seven days.

`security:sbom` writes `cargo.cdx.json` (CycloneDX 1.6) from locked, offline Cargo metadata, and
`pnpm.cdx.json` (CycloneDX 1.7) from pnpm's lockfile-only SBOM command. Cargo's resolved workspace graph
includes build/dev/target dependencies, names, versions, license expressions and dependency edges;
absolute local package IDs are replaced by local BOM references. It includes all 675 resolved Cargo
packages at this implementation, rather than claiming to be a per-platform binary inventory. pnpm's
workspace graph currently has 1,004 components. Regenerate both from the final release lockfiles and
attach platform artifact digests/provenance and release signatures in the release pipeline.

## Executable checklist

| Boundary | Automated evidence | Additional acceptance |
| --- | --- | --- |
| Private socket/pipe and discovery | `vellum-ipc-local --lib`; `node scripts/ipc.mjs` | Run Linux/macOS/Windows lane: directory/file modes or DACL, unsafe path refusal, token rotation, auth before version, default no TCP, exact Host/Origin and lifecycle |
| Token handling | SecretString/provider tests; synthetic canary | Real keychain/installed engine/cloud-session checks use the tagged lanes; never use a real credential as a canary |
| Webview CSP/bridge | `pnpm --filter @vellum/extension test`; `security.test.ts` | Installed VS Code webview: scripts nonce, no network/frames/forms, raw RPC/path/command refusal, no secret fields |
| Electron hardening | `pnpm --filter @vellum/desktop test`; sender/asset/browser/update tests | Installed `test:electron` checks sandbox and preload boundaries; no Node, no raw IPC, no webviews/navigation/permissions/downloads, no renderer network |
| Script escape | `cargo test -p vellum-script-quickjs`; `node scripts/script-sandbox.mjs` | Native/WASM parity and escape corpus; imports, eval/constructors, prototype tampering, getters/proxies, memory/loop/recursion bombs and sticky limits |
| Prompt injection | `cargo test -p vellum-engine-builtin --test engine hostile_content` | Corpus covers document notes, context nodes, skill metadata/body, libraries, tool results and summaries; add live-model evals without treating model behavior as authorization |
| Hostile files/transactions | `pnpm security:fuzz`; core `security_fuzz` | Preserve crashes as ordinary regression fixtures; native renderer/image/font/parser campaigns remain separate |
| Secret leakage | `pnpm security:canary` | Zero raw canary hits in repo, existing `~/.vellum`, scripted outputs and logs; incomplete read fails, no production credentials |
| Supply chain | `cargo deny check`; `pnpm audit --json --audit-level low`; `pnpm security:sbom` | Review every advisory and final lockfile/SBOM; no automatic ignore/update/override |
| Update tampering/signing | Desktop signature/digest/target/version tests and packaging gate tests | Supply release version/icon, HTTPS feed, embedded Ed25519 trust key, signing/notarization credentials, then signed install/upgrade on all targets; never infer this from mocks |
| Share link/ownership | Existing API/storage/share tests and explicit model scenarios | Review intended audience, owner checks, dependency list, revocation, private bucket policy and external access-log/referrer/history leakage |
| Compromised MCP client | Initialized/registry/permission tests and threat-model review | `vellum/share` has no distinct host credential; decide/enforce publish approval or host capability before claiming a person-only boundary |

`security:canary` extends the model-provider runner with a real built-in turn over a synthetic loopback
provider. A fresh ephemeral key lives in a fake SecretStore and is sent only to that test transport.
The scenario draws/checks a frame, stores conversation/steps and images in SQLite/cache, records a
permitted export write and a denied network request, produces an exported artifact and a rendered share
bundle, and records diagnostics/captured frames. It uses production paths over fake document filesystem
and permission prompts. There is no live cloud upload, real OS keychain, account or installed engine.

The runner redacts its own console output, persists raw test logs only in a disposable directory for
scanning and removes that directory afterward. After successful tests it scans the repo and existing
`~/.vellum` as well. On test failure it scans only disposable test artifacts, reports incomplete coverage
and fails. Scanner symlink traversal is intentionally disabled; inaccessible files fail the scan. The
same-user/OS trust assumptions in the model still apply. Do not upload raw logs from a failing canary.
Large native build trees make the complete scan expensive; let it finish before claiming zero leaks.

## Fuzzing

Needs cargo-fuzz 0.13.2, LLVM/C++ and a sanitizer-capable nightly (Unix x86-64 or ARM64). CI installs
`nightly-2026-10-02`; a local installed `nightly` is the default. To choose the CI compiler locally:

```sh
node scripts/security.mjs fuzz nightly-2026-10-02
```

The runner uses an isolated `target/security-fuzz` build, seed 69, at most 1,000 executions/20 seconds
per target after compilation, 8 KiB inputs, a 10-second per-input timeout and a 2 GiB RSS limit.
An optional final integer increases the execution cap (1–10,000,000), for example
`node scripts/security.mjs fuzz nightly 1000000`; the time cap still applies.
Builds are bounded to 15 minutes per target. This is a smoke campaign, not exhaustive fuzzing.

| Target | Production behavior and oracle |
| --- | --- |
| `decode` | Hostile JSON admission; successful decode must encode/decode to stable bytes |
| `merge_driver` | Driver's decode/check/three-way merge/encode path on raw byte branches; merged result must be valid and canonical; no filesystem replacement is performed |
| `merge` | Fixture-based generated branches; diff replay, merge validity/identity and clean commutativity |
| `op_application` | Mixed valid/invalid batches; planning preserves input, bad tail refuses whole batch, accepted edits match plan, exact inverse restores document and schema validity |
| `script_header` | Production script header parser on arbitrary UTF-8 |
| `daemon_frames` | Exact production incremental frame reader with 1–256-byte buffers; split, coalesced, truncated, oversize and empty streams plus hello decoding; no listener/real credential |

Seed corpora and generated failures are under `target/security/corpus/` and `target/security/artifacts/`.
Codec seeds include valid document fixtures; op smoke runs on every ordinary core test without nightly.
The byte merge target supplements the existing semantic merge target; it does not test Git filesystem
replacement. The frame target tests framing/hello parsing; authenticated handshake/HTTP behavior stays
in IPC tests. Reproduce a crash using cargo-fuzz's printed command, minimize it and add a fixed regression
before fixing production code. Never lower a limit or discard a crashing seed just to obtain a pass.

## Current release concerns

The local full advisory checks report 14 existing Rust advisories, and five JavaScript advisories
(two high, three moderate): smol-toml, braces and sprintf-js. The latter two have no patched version
reported by the audit endpoint. See `target/p69-cargo-deny.log` and `target/p69-pnpm-audit.json` for this
run; new runs must use fresh databases. Bans/licenses/sources and dependency graph rules pass. No
advisory ignore or dependency change was made by this prompt.

Native editable host acceptance, separately authorized share publication, real signing/keychain/cloud
provisioning, Windows/Linux installed acceptance and remote CI remain open. Existing visual/contrast
release blockers in reports.md also remain. This checklist is not a claim that the release is accepted.
