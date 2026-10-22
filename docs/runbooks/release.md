# Release pipelines

Prompt 72 implements the release machinery. It does **not** establish public-beta readiness.
`scripts/release-policy.json` retains unresolved native-host, permission, Stop/performance, parity,
visual and contrast acceptance. Existing advisory and repository-policy failures also block their
required CI jobs. Nothing was signed with a production identity, uploaded or published locally.

`pnpm release:status` validates the inventory and prints all sixteen canonical blockers, evidence
paths and remaining acceptance notes. It deliberately never reports local inventory as release
authorization. Fix the underlying behavior and run its acceptance before removing a note. Do not
change tolerances, skip tests or replace failed measurements to obtain a green release.

## Candidate evidence

`pnpm release:gate` is a read-only GitHub API check. The Release workflow supplies its repository,
commit and read-scoped token. It requires every named job in the policy, at the **same commit**, in
the latest trusted run of each workflow. Missing, stale (seven days), failed, cancelled, skipped,
fork and PR evidence does not authorize release. In-progress/newer failed reruns cannot fall back
to an older pass. API errors, missing jobs and truncated pagination fail closed. Jobs are read from
the current run attempt. The gate is repeated immediately before publication.

| Workflow | Required evidence |
| --- | --- |
| `ci.yml` | Rust fmt/clippy/tests, JS format/type-check/tests, dependency rules and advisories, clean room, generated TS/schema, design/tokens, layout and text oracles, parity inventory |
| `renderer.yml` | Six native targets, WASM/WebGL2, six real hardware GPU lanes, original render/performance gates |
| `security.yml` | Boundaries/advisories, fuzz regressions and the scripted session canary |
| `script-sandbox.yml` | Native/WASM escape and determinism corpus |
| `engines.yml` | Nightly/manual Claude Code and Codex, using provisioned trusted runners |
| `hosts.yml` | VS Code and Electron on macOS/Windows/Linux, each x64 and arm64 |
| `release-acceptance.yml` | Strict v1 parity, approved design snapshots, UI/edge/WCAG/RTL tests with zero skips/flakes |
| `performance.yml` | Same-machine base/candidate native and browser comparisons plus absolute budgets |

The machine-specific lanes remain tagged and separate from ordinary unit tests. Dispatch renderer
acceptance with `hardware_gpu=true` and paired performance with a reviewed `base_ref` at the candidate
ref. Installed engines require `VELLUM_ENGINES_ENABLED=true`; skipped jobs remain release failures.
The strict parity command intentionally fails while the existing inventory contains pending v1 rows.
CI's ordinary parity inventory stays useful during development without claiming those rows passed.

## Design snapshot bundle

Export snapshots **locally** using the existing read-only design runbook. CI never runs pen.dev or
receives its credentials. `node scripts/release.mjs snapshot-pack target/design-snapshots.json`
packs the inventory and every named PNG deterministically and refuses to overwrite output. Review
the bundle digest and commit it as `designBundleSha256` in the policy. The current local bundle has
748 PNGs and SHA-256 `5fe1859c47f6d59b75478ca4aacb04c5f10df713626406340c9f6d70c2f0c40e`.
It is a generated, gitignored artifact at `target/design-snapshots.json` (about 176 MiB).

The operator uploads that file to the repository's release named `design-<inventory.source_sha256>`.
The acceptance workflow downloads it through the read-scoped GitHub CLI, verifies the committed
bundle hash and exact inventory, then stages only safe inventory-named PNGs. Missing PNGs can no
longer silently skip a release golden. This bundle was made from the existing local exports; it
does not resolve their known visual or contrast failures. Re-exporting changed source requires
regeneration and review of a new digest, never an automatic CI baseline update.

`node scripts/release.mjs design-gaps` rejects dated entries marked blocking. A blocking entry can
be superseded append-only by another entry with the same `check=<key>` and `status=closed` or
`status=resolved`. Prose such as “not resolved” cannot close a finding.

## Skia policy

ADR-016 chose option A: pinned **source-only** Skia with keyed development/CI caches. Prompt 72
retains that decision; it does not introduce a binary archive producer, download URL or checksum
allowlist. `skia-source.yml` reruns the renderer matrix on Skia/config/SDK/toolchain/patch changes.
Keys include target, compiler/SDK identity, Skia version/features/flags, emsdk and patch bytes.
`VELLUM_SKIA_SDK` can name an explicitly selected SDK. macOS otherwise reads its selected SDK,
Windows reads installed SDK versions and Linux reads Clang's version.

Release builds restore no Cargo build cache and use fresh run/attempt-specific target directories.
The wrapper forces source compilation and strips binary-download overrides; binary-cache remains
disabled in Cargo. The shared WASM module is built once and its provenance verified before any
host packages it. CI caches are not release archives. See ADR-032 for the guide's archive-bullet
deviation. The format-spec generator also uses the source wrapper for its codec example; neither
the generated schema nor the read-only design/product specifications changed.

## Provisioning before the first release

1. Finish all open policy notes and failing checks. Complete installed-host and hardware acceptance.
   Commit matching stable, nonzero Cargo workspace, desktop and extension versions and the lockfile.
   npm scope/package ownership and both extension namespaces must belong to the project owner.
2. Protect `main` and release tags, prevent tag movement, and require review of workflow/policy changes.
   Configure required reviewers and tag deployment restrictions for `release-signing` and
   `release-publish`. GitHub does not create these protections from workflow YAML. Permit them only
   for this repository's reviewed tags. Keep fork/PR code off credential-bearing runners.
3. Provision disposable `vellum-release` native runners for macOS/Windows/Linux × X64/ARM64. Each
   needs the pinned Rust/Node/pnpm, native build/Skia tools, Git Bash on Windows, packaging prerequisites,
   and authenticated code-signing identities already in its OS keychain/certificate store. Do not
   reuse a signing workspace that has run PR code. Signing keys do not enter the repository.
4. Set protected variables `VELLUM_MAC_SIGNING_IDENTITY`, `VELLUM_NOTARY_PROFILE`,
   `VELLUM_WINDOWS_CERT_SHA1`, `VELLUM_TIMESTAMP_URL`, `VELLUM_WINDOWS_PUBLISHER`,
   `VELLUM_UPDATE_URL` (HTTPS directory ending `/`) and `VELLUM_RELEASE_PUBLIC_KEY` (Ed25519 PEM).
   The Apple profile is stored by the operator in the runner's keychain. Windows certificates must
   match the configured publisher. Linux authenticates downloads through the signed inventories.
5. Inject `VELLUM_RELEASE_PRIVATE_KEY` through the signing environment's secret provider. Scripts
   receive it on stdin, validate it against the pinned public key, never save it, and omit it from
   child packer environments. The private key and certificates are not workflow artifacts.
6. Configure npm trusted publishing for all seven packages, bound to this repository's `release.yml`
   and `release-publish` environment. The GitHub-hosted publisher uses Node 24 and npm >=11.5.1,
   publishes provenance, and needs no long-lived npm token. Provision `VSCE_PAT`, `OVSX_PAT`,
   `HOMEBREW_TAP_TOKEN`, and the owned `VELLUM_HOMEBREW_TAP` repository (with a `Formula/` directory).
   The Homebrew token needs contents access only to that tap.
7. Provision an S3 update prefix and HTTPS delivery matching `VELLUM_UPDATE_URL`. Set
   `VELLUM_RELEASE_AWS_ROLE`, `VELLUM_UPDATE_REGION`, and `VELLUM_UPDATE_BUCKET` (`s3://bucket/prefix/`).
   Trust GitHub OIDC only for this repository's protected `release-publish` environment, and permit
   writes only to that prefix. No delete or public-read ACL is used. Delivery/retention and actual
   IAM/HTTPS configuration need operator review; this prompt does not provision a cloud account.

The release-only entrypoints run directly with Node, outside Turbo. Their narrowly scoped Biome
environment-rule override avoids adding signing/registry secrets to Turbo's shared environment or
cache inputs; all other lint rules remain enabled. Only the non-secret SDK selector passes through
Turbo to uncached native build/check tasks.

## Build, publish and recovery

Run acceptance for the exact candidate, create its protected `vX.Y.Z` tag, then explicitly dispatch
**Release** at that tag. The gate requires the tagged commit to be in reviewed main history and all
required CI evidence to pass within seven days. A normal build/PR/tag push does not publish.

The workflow builds a fresh source-only WASM module, then six matching native CLI/daemon binaries.
macOS uses Developer ID signing, explicit signature verification and an Accepted notarization
receipt. Windows uses the certificate store and Authenticode verification. Existing packaging
creates VSIX and signed Electron installers without publishing. Linux needs no Apple credentials.
`VELLUM_DESKTOP_ACCEPTANCE=complete` is supplied only after the candidate gate, not as its substitute.

Each target produces a CLI tarball, VSIX, native npm tarball and Electron feed tarball. Electron
bundles contain installers, blockmaps, the existing Ed25519 update envelope and latest metadata.
The npm wrapper has six exact optional platform packages, no postinstall/download fallback, and
forwards arguments, signals and exit status to the shipped binary. Linux packages declare glibc.
The Homebrew formula pins all four Darwin/Linux archives to exact SHA-256 digests.

Assembly requires all 24 native outputs, wrapper, formula, two source dependency SBOMs and the
candidate receipt: 29 payload files. It verifies native build attestations, then signs the complete
version/commit/filename/size/SHA-256 inventory. The publisher rechecks the candidate, verifies that
the remote tag still resolves to the signed commit, verifies the exact payload set/signature and
source/workflow attestations, and obtains the scoped S3 role before any registry writes.

Publication creates a GitHub draft without overwriting a release, publishes six native npm packages
before their wrapper, publishes six VSIX files to both registries, updates the owned tap, deploys
the Electron feeds, then exposes the GitHub release. Feed extraction requires regular flat files
and reuses the installed app's signature verification; hashes/sizes are checked before upload.
Payloads upload before signed envelopes and latest pointers; mutable metadata is not cached.

Registry publication is **not atomic**. If any step fails, the GitHub release stays a draft and
already-published registry versions may exist. Inspect each registry and its signed inventory before
recovery; never force overwrite a version, downgrade a feed or rerun blindly. Feed publication can
briefly leave mismatched metadata after a partial upload; installed clients refuse it and keep their
current version. Finish the same verified feed deployment or publish a newer reviewed release.
Keep previous payloads and signing roots available for recovery; do not delete installed versions.

No live registry, signer, notarization service, update bucket or installed upgrade was exercised by
the local tests. Matrix execution, protected environment configuration, real signatures and installer
updates/rollback remain acceptance work for the operator.

Primary tool references: [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating),
[npm trusted publishing](https://docs.npmjs.com/trusted-publishers),
[Electron signing](https://www.electron.build/code-signing.html),
[AWS OIDC action](https://github.com/aws-actions/configure-aws-credentials/tree/v4.3.1).
