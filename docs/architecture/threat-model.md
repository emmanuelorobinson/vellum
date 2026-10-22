# Vellum threat model

Prompt 69, 2026-10-22. Repository source model, not a completed penetration test or release approval.
This file is the output explicitly requested by Prompt 69; other architecture guidance is unchanged.
An independent, read-only architecture review informed this model; material claims were checked against
their consumers. Deployment facts not supplied by the owner remain unknown.

## Overview

Vellum opens repo-backed design documents through native or WASM adapters. The daemon serves CLI/MCP
clients over private local IPC. Core operations validate and attribute edits; the renderer and script
runtime consume document-derived input. Desktop and VS Code webviews use native-host bridges.
Optional account/API, snapshot sharing and sync are separate network services. Native document hosts
currently remain read-only pending the document ABI/EditorLink integration recorded in reports.md.

| Component | Entry and enforcing source |
| --- | --- |
| JSON and library input | `crates/adapters/codec-json/src/decode.rs:37`, `crates/adapters/assets-fs/src/std_fs.rs:7` |
| Document writes | `crates/vellum-core/src/ops/mod.rs:601`, `crates/vellum-core/src/ops/mod.rs:770` |
| Script execution | `crates/adapters/script-quickjs/src/lib.rs:91`, `crates/vellum-core/src/script_nodes/mod.rs:31` |
| Model context | `crates/adapters/engine-builtin/src/pack.rs:11`, `crates/adapters/engine-builtin/src/lib.rs:238` |
| Outside actions | `crates/vellum/src/permissions.rs:47`, `crates/vellum-core/src/permissions/mod.rs:63` |
| Local transport | `crates/adapters/ipc-local/src/server.rs:35`, `crates/adapters/ipc-local/src/private.rs:27` |
| Native hosts | `apps/desktop/src/main/security.ts:1`, `apps/extension/src/security.ts:24` |
| Updates | `apps/desktop/src/main/update-driver.ts:9`, `apps/desktop/src/main/release-signature.ts:24` |
| Optional cloud | `apps/api/src/index.ts:135`, `packages/storage/src/index.ts:177`, `crates/sync-server/src/lib.rs:153` |

Sensitive resources have distinct deployment boundaries. Paths describe locations, never credentials.

| Deployment or workflow | Resource or capability | Configuration and precedence | Safe effective value or location | Readers, writers, or recipients | Enforcing control | Evidence or unknowns |
| --- | --- | --- | --- | --- | --- | --- |
| Default daemon | IPC discovery and bearer rendezvous | HOME/USERPROFILE, `.vellum`, `run` | `~/.vellum/run/daemon.json`; Unix `vellum.sock`; Windows user-SID/run-directory-hash pipe | Daemon and same-user clients | 0700 directory, 0600 files/socket, Windows protected user DACL, remote pipe refusal, fresh token | `crates/adapters/ipc-local/src/private.rs:27`, `crates/adapters/ipc-local/src/windows_security.rs:183` |
| Explicit runtime | Session, recovery and share state | `--run-dir`; state is its parent | `<parent(run-dir)>/sessions.db`, `cache/`, `recovery/`, `shares/<path-hash>.json` | Native daemon and UI | Subsystem private-file checks; run-dir ACL alone does not prove parent privacy | `crates/vellum/src/main.rs:94`, `crates/vellum/src/sessions.rs:31`, `crates/vellum/src/mcp/share.rs:5` |
| Explicit daemon TCP | RPC authority | `--listen`, exact allowed origins | Supplied bound address, `POST /rpc`; absent by default | Bearer clients | Exact Host/Origin, bounded body/headers/deadline, 64-client admission | `crates/adapters/ipc-local/src/server.rs:41`, `crates/adapters/ipc-local/src/server.rs:266`; no built-in TLS claim |
| Local documents/libraries | Read files and dependencies | Absolute document; nearest repo root or document parent; relative paths | `<root>/<checked-relative-path>` | Codec, asset/library/script loaders | Lexical path checks; ordinary StdFs follows symlinks | `crates/vellum/src/daemon/production.rs:327`, `crates/adapters/assets-fs/src/std_fs.rs:7` |
| Share collection | Publish dependency bytes | Separate share filesystem wrapper | Frozen bundle and private spool under state/shares | Uploader and configured cloud | Reject hidden files/symlinks, bounded reads, digest binding, four upload workers | `crates/vellum/src/daemon/production.rs:455`, `crates/vellum/src/daemon/share.rs:508` |
| Local provider | API and cloud-session credentials | Interactive CLI Keychain | Provider key reference in OS keychain | Selected authenticated transport | SecretString redaction and adapter error classification | `crates/vellum/src/cli/prompt.rs:663`, `crates/adapters/secrets-keyring/src/lib.rs:28` |
| Explicit CI provider | API credential | CI=true/1 selects EnvSecrets directly | VELLUM provider API-key env reference, then short alias | CI process/provider | Read-only env adapter; CI secret injection is an operator boundary | `crates/vellum/src/cli/prompt.rs:663`, `crates/adapters/secrets-keyring/src/lib.rs:60` |
| Installed engines | External process authority | Selected executable/cwd and explicit environment allowlist | Installed engine's own config/credentials | Trusted installed executable | Environment clearing/allowlist, bounded pipes, process-tree cancellation | `crates/adapters/engine-codex/src/native.rs:24`; not an OS process sandbox |
| Packaged desktop | Update install authority | resources/desktop-release.json supplies feed and trust key | Signed platform/architecture manifest and hashed artifacts | Native updater | Ed25519, size/SHA-512, version/target checks, verified-download gate | `apps/desktop/src/main/update-driver.ts:38`, `apps/desktop/src/main/update-driver.ts:82`; actual feed/signing custody unknown |
| Optional API/account | Auth, DB, mail, billing and R2 secrets | Composition environment through wrappers | Configured HTTPS origin/Postgres/R2; API default loopback:3001 | Server service principals | Trusted origins, secure cookies with HTTPS, bearer-over-cookie identity, owner and entitlement checks | `apps/api/src/services.ts:14`, `apps/api/src/server.ts:14`, `packages/auth/src/index.ts:48` |
| Public share read | Audience bearer capability | Published random share ID | `shares/<id>`, `snapshots/<uploadId>` | Anyone holding link | Private storage, authenticated mutations, digest/size checks, revocation, no-store/no-referrer | `packages/storage/src/index.ts:93`, `packages/storage/src/index.ts:240`, `apps/api/src/index.ts:212`; external logs/ACLs unknown |
| Self-host sync | Local DB/snapshots | `--data` defaults `.sync-data`; compose overrides `/data`, mount from VELLUM_SYNC_DATA | `<data>/sync.sqlite` and `<data>/<digest>.vellum`; compose host port loopback:7310 | Sync service and authorized room clients | Private directories, roles, size/digest checks; nonroot/read-only container | `crates/sync-server/src/main.rs:130`, `crates/sync-server/compose.yml:7`, `crates/sync-server/Dockerfile:21` |
| Cloud sync | Room log/presence/snapshots | Explicit `--cloud`; DB/Redis env; S3 config plus keychain storage refs | Configured Postgres/Redis, bucket `sync/snapshots/<digest>.vellum` | Service principal and room participants | Expiring room roles, actor-bound import, validated endpoints | `crates/sync-server/src/main.rs:212`, `crates/sync-server/src/lib.rs:153`; WSS/provisioning unknown |

## Threat Model, Trust Boundaries, and Assumptions

Protect document integrity and attributed undo/history; repository files and host process authority;
private design content in sessions, images, recovery and share spools; provider/session/sync credentials;
room/account ownership; and the integrity of installed applications and their update trust roots.

Attackers may provide files, libraries, fonts, images, scripts or document text that a person opens.
They may control an explicitly configured skill source, a model response, imported remote HTML,
an unauthenticated local client, an authenticated compromised MCP client, an update transport or a share
link. They do not thereby possess the owner's OS account, keychain, release signing key or cloud role.
An already same-user process can read private user state: ACLs do not isolate mutually hostile programs
running as that user. A client holding the daemon credential already has the common authenticated RPC
interface. No stronger person/host identity is established by hiding an RPC method from tools/list.

Required objectives are the guide's invariants 1–5 and 8–12: only validated Ops mutate the document;
edits retain actor/session/turn attribution and undo semantics; paused/stopped agents cannot commit;
scripts have bounded resources and only their supplied API; local transport is private by default;
native provider credentials stay out of documents, transcripts and public outputs; outside actions
need action-specific recorded permissions. Prompt text is never a permission record.

Prompt 69 removes skill metadata from system interpolation. The engine attaches an escaped
`untrustedSkills` JSON catalog to the user's request, preserves tool-result provenance, and adds an
instruction policy to normal and summary requests. The shipped prompt describes context-node
authorship as unverified. These are defense in depth; the model is not an authorization mechanism.
Tests use scripted responses to prove structural isolation and dispatch behavior, not empirical
resistance of every live model. Installed engines retain their own instruction hierarchy.

Distinct controls and limitations:

- The codec rejects malformed/duplicate/unsupported data; semantic document validation is separate.
  Fuzzing a decoder does not prove arbitrary native render inputs safe or establish a universal file budget.
- Script nodes have an input-only API and return derived nodes. Execute has a different API and commits
  via the op path. QuickJS exposes no native filesystem/network API; the supplied ScriptApi determines
  its effective authority. Native and WASM need the same escape/limit corpus.
- PermissionBroker canonicalizes actions, records decisions and supplies scoped permits. Explicit
  skip-all and installed-engine auto modes change the person's policy and must stay visible.
- API/session keys, IPC rendezvous credentials, explicit CI environment credentials and cloud-server
  secrets are different resources. AGENTS.md's broad keychain-only wording does not describe all four
  actual stores; this is a documented implementation/policy distinction, not evidence of leakage.
- `vellum/share` currently checks initialized client state, not a separate person-origin credential
  (`crates/vellum/src/mcp/share.rs:9`, `crates/vellum/src/mcp.rs:503`). A host-only comment is insufficient
  authorization. A distinct host capability or broker-enforced publish approval remains a release concern.
- Ordinary StdFs follows symlinks. Lexical repo roots are not physical confinement. The share collector
  owns stronger checks; sensitive outside actions must still use canonical broker paths.
- Share links intentionally grant public read/download to their holders. Revocation blocks future reads
  through the service, and cannot recall downloaded copies. Bucket ACLs, proxy logs, backup retention,
  production TLS/WSS and signing custody require deployment acceptance.
- Writable native EditorLink, account room-capability issuance and integrated native outbox persistence
  are incomplete. No backend unit test establishes those installed-host guarantees.

## Attack Surface, Mitigations, and Attacker Stories

These are prioritized threat scenarios, not a list of newly validated vulnerabilities.

| Priority | Scenario and capability gain | Prerequisites | Impact | Existing controls | Mitigation | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| High | Malicious .vellum or library drives invalid graph, deep decode or costly rendering | Person opens/imports file | Crash/resource exhaustion; potential parser/native corruption | Typed codec, duplicate/version rejection, core validation | Decode/merge/op fuzzing, scoped render budgets; retain discovered input as regression | `crates/adapters/codec-json/src/decode.rs:81`, `crates/vellum-core/src/ops/mod.rs:601` |
| High | Script node/execute escapes API or evades limits | Script evaluated | Host I/O or denial of service | Fresh realm, allocator/stack/clock/op/node limits, allowlist and JSON boundary | Native/WASM escape corpus, sticky failures, no new host globals | `crates/adapters/script-quickjs/src/lib.rs:113`, `crates/adapters/script-quickjs/src/lib.rs:224` |
| High | Document/skill/context/summary text asks for credentials or fabricated approval | Model consumes attacker text | Unauthorized outside actions or publication | Static system policy, untrusted data messages, scoped permits | Keep broker/host identity independent of text; negative corpus and live-model evals | `crates/adapters/engine-builtin/src/pack.rs:11`, `crates/vellum/src/permissions.rs:85` |
| High | Compromised MCP client calls unpublished host RPC | Possesses local daemon token and initializes | Common local interface authority, including share actions | Authentication and owner credentials at cloud API | Enforce separate host/publish approval capability before claiming person-only methods | `crates/vellum/src/mcp/share.rs:9`, `apps/api/src/index.ts:135` |
| High | Renderer/webview escape acquires native capability | Compromised document content or renderer | Arbitrary file/process/network access | Electron sandbox/isolation/no Node; exact sender, typed requests, asset allowlist, CSP, navigation/permission/download denial; extension nonce CSP | Maintain negative bridge/CSP tests and installed Electron/VS Code lanes | `apps/desktop/src/main/main.ts:776`, `apps/desktop/src/main/security.ts:4`, `apps/extension/src/security.ts:53` |
| High | Update transport substitutes executable or manifest | Owner checks/downloads update; no signing key theft assumed | Persistent application compromise | Embedded Ed25519 trust, target/version/file/hash/size binding, install only after verified bytes | Sign/notarize actual release, protect signer and feed, test corrupt/replayed/foreign artifacts | `apps/desktop/src/main/release-signature.ts:24`, `apps/desktop/src/main/update-driver.ts:82` |
| Medium | Other local user attacks discovery/socket; arbitrary frames | OS-local access without owner credential | Unauthorized RPC or daemon exhaustion | Private path/ACL, auth before version, bounded frame/handshake/concurrency | Platform IPC tests, split/truncated/oversize frame fuzzing; reject unsafe existing paths | `crates/adapters/ipc-local/src/private.rs:157`, `crates/adapters/ipc-local/src/server.rs:107` |
| Medium | Library dependency uses symlink to read outside intended root | Project supplies link and loader follows it | Unexpected file disclosure | Lexical checks; canonical broker outside path; separate restricted share wrapper | Audit each library/asset consumer's physical path and approval, never assume StdFs confinement | `crates/adapters/assets-fs/src/std_fs.rs:7`, `crates/vellum/src/permissions.rs:47` |
| Medium | Share link leaks through forwarding/history/logging | Attacker obtains capability URL | Snapshot confidentiality loss | Random ID, no-referrer/no-store, private objects, revocation | Limit audience; inspect proxy/CDN logs and retention; confirm intended dependencies before publishing | `apps/api/src/index.ts:212`, `packages/storage/src/index.ts:240` |
| Medium | Account or sync client crosses ownership/room roles | Optional services deployed, valid low-privilege identity | Other owner's data or attributed edits | Bearer/cookie identity selection, origin checks, owner checks, room role/expiry and actor import | Cross-owner/room tests; WSS and secret/capability provisioning acceptance | `apps/api/src/index.ts:31`, `crates/sync-server/src/lib.rs:153` |
| Medium | Dependency/build tool or signing pipeline is compromised | Dependency or privileged CI/release path | Build/runtime compromise | Exact product versions, lockfiles, cargo-deny, clean-room policy, existing signing gates | pnpm audit, source SBOMs, least-privilege workflow, verified release artifacts; no advisory suppression | `scripts/check-deps.sh:90`, `apps/desktop/sign-release.mjs:26` |

Executable checklist, commands, evidence and release acceptance are in `docs/runbooks/security.md`.

## Severity Calibration (Critical, High, Medium, Low)

- **Critical:** attacker-controlled ordinary file or unauthenticated public request yields broad host
  execution or cross-account compromise with no approval. Native corruption/reachability must be proved;
  a decoder panic or advisory title alone does not establish this impact.
- **High:** verified sandbox/native-bridge escape, bypass of a required outside-action approval, or an
  attacker-supplied update accepted without the installed trust key. Explicit same-user credential
  possession and user-selected auto policy change the starting authority and severity.
- **Medium:** bounded but material parser/daemon denial of service, unintended dependency read with
  reachable prerequisites, or exploitable share-link disclosure across a distinct audience boundary.
  Public reading with an intentionally supplied share link is expected behavior.
- **Low:** limited exposure or hardening weakness requiring unusual prerequisites and little new
  authority. An unchecked assumption is an open question, not automatically a low-severity finding.

Confidence, deployment prerequisites and impact remain separate. No production endpoints, real keys,
installed signing acceptance, independent model safety guarantee or completed full security audit is claimed.
