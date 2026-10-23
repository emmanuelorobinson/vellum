---
title: "Version policy"
description: "How the project chooses and records the versions of its tools and dependencies, and why a prerelease is never used in a lockfile."
---

The project pins versions exactly, so a clean checkout builds the same way for everyone. A range such as `^1.2` is not used for a shared tool.

## The rules

1. **The latest stable patch of each line.** A tool is pinned to the newest stable release of the line the guide names. A prerelease, such as a release
   candidate, alpha or beta, is not used.
2. **Exact versions everywhere they are declared.** A package manifest pins an exact version, and the workspace's catalog holds the shared versions, so
   each package refers to the same one. Rust requirements use the caret form that cargo writes, and the lockfile holds the exact version it resolved.
3. **No prerelease in a lockfile.** A test fails if a lockfile contains a prerelease.
4. **Recorded before installed.** A version is written into the version baseline before it is installed, along with the build step that needs it. Nothing is
   installed ahead of the step that needs it.
5. **Release age.** The package manager's minimum release age applies to new versions. Its exceptions are listed in the workspace file, and each one names the
   version it excepts.

## Where versions are recorded

| Record | What it holds |
| --- | --- |
| `docs/architecture/version-baseline.md` | Every pinned version, its line in the guide, the decision it supports, and the date it was checked |
| `pnpm-workspace.yaml` catalog | The shared package versions |
| `Cargo.toml` and `rust-toolchain.toml` | The Rust toolchain and the crate requirements |
| `.nvmrc` | The Node version |

The version baseline is checked by a test. The test fails when a version in a manifest disagrees with the baseline, or when a line of the guide has no version.

## Adding a version

Check that the version is the latest stable one of its line, add it to the baseline with its decision, add it to the catalog or the manifest, and run the
install. Then run the version test. The change to the baseline is reviewed with the change that needs the version.
