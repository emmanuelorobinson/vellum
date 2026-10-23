---
title: "Environment variables"
description: "The environment variables {{brand}} reads, grouped by who sets them: you, a continuous-integration system, the developers who build it, and the operators of its services."
---

These are the environment variables the code reads, grouped by who sets them. A variable not listed here is not read by this version.

## For you

| Variable | Read by | Meaning |
| --- | --- | --- |
| `VELLUM_CHROME` | Browser capture, and the developer checks that use a browser | The path to a Chromium executable |
| `VELLUM_ACCOUNT_URL` | `vellum login` | The account service's origin, used when no saved origin exists |
| `VELLUM_MODEL_ENDPOINT` | `vellum prompt` | The full endpoint for the chosen provider. Must be HTTPS, or HTTP on loopback |
| `VELLUM_RUN_DIR` | The VS Code host and its tests | An override for the daemon's run folder |

## For continuous integration

These apply only when `CI` is set to `1` or `true`. Otherwise keys come from the keychain.

| Variable | Meaning |
| --- | --- |
| `CI` | Set to `1` or `true` to read provider keys from the environment |
| `VELLUM_<PROVIDER>_API_KEY` | A provider's key, where `<PROVIDER>` is the provider id in upper case with hyphens replaced by underscores. For example, `VELLUM_OPENROUTER_API_KEY` |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY` | The conventional names, also read in CI |

An environment key is never written to the keychain, a design file, a session, a usage report or a log.

## For developers

| Variable | Used by |
| --- | --- |
| `VELLUM_SKIA_SDK` | The native build, to name an explicit SDK |
| `VELLUM_SKIA_FROM_SOURCE` | The native build, to force a source build of Skia |
| `VELLUM_LANES` | The test scripts, to choose which tagged lanes run |
| `VELLUM_VSCODE_EXECUTABLE` | The VS Code tests, to name the VS Code to test with |
| `VELLUM_ELECTRON_EXECUTABLE` | The desktop tests, to name the Electron executable |
| `VELLUM_CLAUDE_CODE`, `VELLUM_CODEX` | The installed-engine tests, to name the executable to test |
| `VELLUM_IPC_BINARY` | The IPC test lane, to name the daemon binary it runs |
| `VELLUM_CLIENT_HOME` | The VS Code host, to point client configuration at a test folder |
| `VELLUM_PEN_CLI`, `VELLUM_DESIGN_OPENER` | The design tooling, to name the design tool's command line and its opener |

## For operators

| Variable | Used by |
| --- | --- |
| `VELLUM_RELEASE_PRIVATE_KEY` | The release scripts. It is read from standard input, and never saved |
| `VELLUM_UPDATE_URL`, `VELLUM_UPDATE_PUBLIC_KEY_FILE` | The release scripts and the packaged app's update check |
| `VELLUM_WINDOWS_PUBLISHER`, `VELLUM_WINDOWS_CERT_SHA1`, `VELLUM_TIMESTAMP_URL` | Windows signing |
| `VELLUM_SYNC_DATA` | The sync server's data folder, in its compose file |
| `VELLUM_POSTHOG_TOKEN`, `VELLUM_POSTHOG_ORIGIN`, `VELLUM_SENTRY_DSN` | The account service's observability, when an operator enables it |

The [release runbook](/docs/operations/runbooks/release) and the [sync runbook](/docs/operations/runbooks/sync) name the values each
of these expects, and where the operator keeps them.
