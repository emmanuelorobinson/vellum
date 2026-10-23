---
title: "Configuration"
description: "Where {{brand}} reads its settings in this version, what each setting is for, and the configuration file the architecture plans but this version does not read."
---

{{brand}} has no single configuration file in this version. Settings come from four places: the command-line options you pass, a
settings file that the image commands write, the operating system's keychain, and environment variables. This page lists each one.

## Command-line options

Most behaviour is chosen per command: the engine, the model, the effort, the run folder and the output format. See the
[command line](/docs/reference/cli) reference.

## The image settings file

`vellum asset-settings` saves the image settings in a file beside the daemon's state, in `~/.vellum`. The file holds the
non-secret settings only:

| Setting | Meaning |
| --- | --- |
| The image provider | An endpoint and a model, given together. The provider must accept the GPT Image-compatible JSON request and return one base64 PNG |
| The segmentation model | The absolute path to an ONNX model for local background removal. {{brand}} does not download a model |

The next daemon start reads the new settings. An invalid file fails explicitly, so fix the file or remove the setting.

## The keychain

Secrets are stored in the operating system's keychain under the service `vellum`. The accounts used are:

| Account | What it holds |
| --- | --- |
| Each provider's id | That provider's API key |
| `figma_token` | A Figma token for REST import |
| `image_model` | The image model chosen for generation |
| `stock.pixabay` | The stock-photo provider's key |
| The account service's origin and session | The device sign-in for Pro features |

Nothing in this list is stored in a design file, a session, a log or a usage report.

## Environment variables

Environment variables cover credentials in continuous integration, the location of a few helpers, and a few overrides for tests and
packaging. The [environment variables](/docs/reference/environment-variables) reference lists each one.

## The planned configuration file

The architecture describes a configuration file, `~/.vellum/config.toml`, with sections for the engine, rendering, the AI engine,
agents, permissions, assets and telemetry. This version does **not** read that file. The shape is shown here so you know what is planned,
and so that you do not write one expecting it to take effect:

```toml
# Planned. Not read by this version.
[ai]
engine   = "builtin"
provider = "openrouter"
model    = "deepseek/deepseek-v4.1-flash"
effort   = "medium"

[permissions]
read_repo    = "ask"
write_files  = "ask"
run_commands = "ask"
```

Until the file is read, choose these values with the command-line options and the settings described above. The
[permissions and safety](/docs/using-vellum/permissions-and-safety) page explains the defaults that apply without it.
