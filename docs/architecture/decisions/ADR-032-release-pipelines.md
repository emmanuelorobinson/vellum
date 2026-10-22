# ADR-032: candidate-bound release evidence and source-only native packaging

- Date: 2026-10-22.
- Status: implemented locally; production signing, remote lanes and publication remain unverified.
- Scope: Prompt 72. Extends ADR-016 and ADR-031 without changing document/runtime behavior.

## Decision

Treat the guide's sixteen release blockers as a checked policy, with real candidate evidence rather
than a hand-maintained “ready” flag. Require the latest trusted workflow run and every required job
at the exact source commit; reject missing/skipped/failed/stale/fork/PR evidence and retain explicit
unresolved acceptance. A stable tag and matching non-placeholder versions are necessary. Recheck
the gate and remote tag before protected publication. CI status alone does not erase known gaps.

UI release acceptance imports a locally exported design bundle pinned by a committed SHA-256 and
the unchanged inventory, and permits no skipped/flaky browser tests or pending v1 parity rows.
Design service credentials and export tools stay out of CI. Existing tolerances remain unchanged.

Native release builds use fresh run-specific target directories, with no restored compiled cache.
The source-built shared WASM module and every native distribution carry source/workflow attestations.
OS keychains/certificate stores supply macOS and Windows signing identities; Linux distributions
are authenticated by detached Ed25519 inventory/update signatures. The complete payload inventory
binds version, source, filenames, sizes and SHA-256. npm has no installation-time download script.

Publication is explicit workflow_dispatch at a reviewed protected tag, with separate protected
signing and publishing environments. Registry and infrastructure credentials are provisioned by the
owner; writing workflow YAML cannot establish environment protection or namespace ownership.
The update bucket uses a scoped OIDC role. Normal PR/build/tag-push workflows have no publish path.

## Deliberate guide difference

Prompt 72 describes a Skia binary-archive publisher. ADR-016 selected guide option A (source builds),
so that publisher/consumer is **not** introduced. `skia-source.yml` reruns the existing source matrix
when Skia, SDK/toolchain, flags or patches change; the build identity includes the compiler/SDK.
The release wrapper forces source builds, disables upstream binary-cache consumption and strips
download overrides. No archive endpoint or unverified binary enters linking. Option B still needs
its own measured proposal and committed-checksum consumer before it can replace this policy.

## Costs and limitations

Fresh release compilation costs more time. Seven-day, exact-commit evidence requires rerunning
expensive GPU, native, engine and host lanes after a candidate change. Public beta remains blocked
by prior native ABI/host, permission, Stop/performance, parity, design/contrast, advisory and repository
policy failures; the implementation is not evidence that those acceptance items passed.

Publishing across npm, extension registries, Homebrew, update hosting and GitHub is not atomic.
The GitHub release remains a draft until the other distributions succeed. Partial failure requires
inspection and controlled recovery rather than version overwrites. Installed updater signatures
refuse inconsistent metadata and keep the existing version. See `docs/runbooks/release.md`.

## Reference study

No §3.2 reference row names Prompt 72, and no reference source was ported. Sibling Outpost maps
explicit open/unproven evidence to release blockers; Cutesycal executes a blocker-to-suite map.
Vellum retains explicit incompleteness, then binds acceptance to trusted same-commit GitHub jobs
and signed exact artifact bytes. It avoids treating test-file existence or a manual boolean as a pass.
Context7 official docs and installed tooling informed Actions, npm, Electron, VSCE and AWS usage.
