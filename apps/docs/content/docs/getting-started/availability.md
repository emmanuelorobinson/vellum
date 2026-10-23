---
title: "Availability"
description: "What is ready to try from source, what is a preview with limits, what needs an account, and what is not available yet."
---

{{brand}} has no signed public release. Everything below is either built from a clone of the repository, or planned.
The statuses come from the repository's runbooks, its tests and its open-items list. When a status here and a runbook
disagree, the runbook's own status note is the detail, and this page is the summary.

## Status at a glance

| Status | Meaning |
| --- | --- |
| Ready to try | Built from source and covered by the repository's tests |
| Preview | Built from source, with a stated limit or a setup step you must supply |
| Needs an account | Works only with a Pro account, and no public account service exists yet |
| Not available | Planned, or partly built, and not usable as a user feature yet |

## The command line and the daemon

| Feature | Status | Notes |
| --- | --- | --- |
| Create, edit and inspect documents | Ready to try | `new`, `open_document`, the read and write tools |
| Export PNG, JPEG, WEBP, SVG and PDF | Ready to try | Limits are in the [export formats](/docs/reference/export-formats) reference |
| Export HTML with CSS or Tailwind | Ready to try | Complex appearance, such as gradients, images and effects, is embedded as an image, and the export reports it |
| Token import and export (CSS variables, Tailwind v4, DTCG JSON) | Ready to try | `--system` is not available, because no design-system catalog service exists |
| Local icons from Lucide, Feather, Phosphor and Material Symbols | Ready to try | Bundled with the build |
| The MCP server, for any supported client | Ready to try | See [connect an agent](/docs/getting-started/connect-an-agent) |
| Scripts through `execute`, and script nodes | Ready to try | The sandbox has no network, no files and no timers |
| Git review as images, and the merge driver | Ready to try | Needs a Git that supports `--no-lazy-fetch`; tested with Git 2.46.2. Works signed out |
| Local daemon, and the isolated run directory | Ready to try | The default directory is private to your user |
| Built-in agent with your own model key | Preview | Needs a provider key in your keychain or, in CI, in a variable. See [providers and keys](/docs/reference/providers-and-keys) |
| Installed Claude Code or Codex as the agent | Preview | Verified on specific versions. Others fail with a clear message |
| Stock photos | Preview | Needs a stock-provider key |
| Generated images and local background removal | Preview | Generation needs a configured provider. Background removal needs a model you install yourself |
| Figma clipboard import | Preview | Covers frames, shapes, text and basic fills. Images and gradients are reported as left out. A live paste has not been verified |
| Figma REST import | Preview | Needs a Figma token. Not verified against the live service |
| Web page capture and element import | Preview | Needs Chromium installed, or the desktop app's Chromium |

## The editors

| Feature | Status | Notes |
| --- | --- | --- |
| VS Code extension: editing `.vellum` files | Preview | Built from source. Editing goes through the daemon. A file the daemon cannot hold opens read-only. Not on the Marketplace or Open VSX |
| Desktop app | Preview | A development build. It is not packaged, signed or updated |
| Desktop Home: recents, drafts and recovery | Preview | Some Home features are not built yet, such as prompt submission from Home |
| Command palette, context menu and settings dialog | Not available | The palette and settings are built in the editor, but the installed hosts do not serve them yet |
| Presenting slides, and slide PDFs from the editor | Not available | The installed hosts still lack the shared document runtime for presenting. The command-line PDF export works |
| Live multiplayer in the editors | Not available | The room service is built. The editors are not attached to it yet |
| Comments | Not available | Planned with live collaboration |
| Localised interface | Preview | Thirteen languages are bundled. Advanced messages are still in English |

## Pro features

| Feature | Status | Notes |
| --- | --- | --- |
| Account sign-in from the command line and the desktop | Needs an account | Device sign-in is built. No public account service exists yet |
| Share links, updated and revoked from the desktop | Needs an account | Built, with the server requirements in the [sharing](/docs/using-vellum/sharing) page |
| Pull request previews | Needs an account | The worker is built. No hosted service exists yet |
| Cloud sync across devices | Not available | The sync server can be self-hosted. No hosted service exists yet |

## Not built yet

- Signed installers, update feeds and a published extension. The [release](/docs/operations/updates-and-releases) page
  lists what must exist first.
- The `~/.vellum/config.toml` file. The architecture describes it, but this version does not read it. Configuration
  uses command-line flags, the settings file the asset commands write, the keychain, and environment variables.
- Figma file import beyond paste, web embeds on the canvas, and generative background replacement. These are listed
  as later work in the product scope.

## What to check before you rely on a feature

Run the feature's own command or test, not only the screen. For a feature marked preview, read its runbook under
[operations](/docs/operations/runbooks), because the runbook records the exact versions and limits it was checked
against.
