---
title: "Quality gates"
description: "The checks that protect each promise {{brand}} makes: the dependency rules, the contract suites, the golden renders, the format round trip, the schema snapshot, the fuzzers and the sandbox corpus."
---

A quality gate is a check that fails the build when a promise breaks. The architecture names the gates below. Each one protects a
specific promise, and each one runs in the repository's own commands, so a contributor can run it before a pull request.

| Gate | What it protects |
| --- | --- |
| Dependency rules | The layered structure: the core depends on nothing outside itself, and vendors appear only in adapters |
| Contract suites, one per port | Any adapter is a valid adapter, because it passes the same suite as the one it replaces |
| Golden renders | The visual output of every renderer, for every fixture, compared against reference images |
| Format round trip | A file loaded and saved again is byte-identical, and every migration has a fixture, so users' files survive upgrades |
| Tool schema snapshot | No accidental breaking change reaches the agents. A changed schema is reviewed as a diff |
| Generated types check | The Rust and TypeScript types cannot drift apart |
| Performance benchmarks | A regression of more than ten per cent in a budgeted measurement fails the run |
| Fuzzing | Malformed files, malformed merges and hostile batches from agents do not crash the engine or corrupt a design |
| Script sandbox corpus | A script cannot reach the network, the files or a timer, and the same script gives the same operations in every environment |
| Tool parity suite | Every tool listed in the parity inventory has a scripted end-to-end test, so no capability disappears silently |
| Agent evaluations | Fixed briefs, run against each engine and model, scored on layout problems, token use and screenshots |

## Where the gates run

Most gates run in the ordinary test command for the package they protect. Some run only in a tagged lane, because they need a real browser,
a real keychain, a GPU or an installed agent. The lanes are named in the [verification](/docs/development/verification) page, and a lane that
was skipped is reported as skipped rather than passed.

## The schema snapshot in practice

The tool schema snapshot is the gate that most often catches a real problem. When a tool's schema changes, the snapshot test fails and shows the diff.
The author then decides whether the change is additive, which is allowed, or breaking, which needs a new tool name. A schema that inlines the
file format would fail a size guard, so the format stays described in the reference, not in every schema.

## Honest results

A gate that cannot run on a platform is reported as not run on that platform. A result is not carried over from one machine to another. The
[performance](/docs/architecture/performance) page shows what the measurements currently say, including the budgets that are not yet met.
