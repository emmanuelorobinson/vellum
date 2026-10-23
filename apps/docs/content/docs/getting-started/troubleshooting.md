---
title: "Troubleshooting"
description: "The problems most people hit when they first build and run {{brand}}, and what to check for each."
---

Most problems come from one of four places: a native build tool, the daemon, a client's configuration file, or a
permission that was refused. Start with the symptom that matches yours.

## The build stops early

**Symptom:** `node scripts/skia-build.mjs build` fails before it compiles anything.

Check the native tools the renderer needs: a C++ compiler, Python 3, ninja, and libclang. On Linux, the OpenGL
development libraries are needed too. The first build fetches and compiles Skia, so it needs network access and a
few minutes. The [renderer runbook](/docs/operations/runbooks/renderer) lists the tools for each platform.

**Symptom:** the build succeeds, but `target/debug/vellum` is missing.

Run the build with `-p vellum`. The build script places the binary under `target/`, not under the crate folder.

## The daemon is offline

**Symptom:** a command says the daemon cannot be reached, or the desktop shell reports the daemon as offline.

- Run `vellum status --json`. It checks the daemon without starting it. A command that works with documents, such as
  `vellum tool get_app_state --json`, starts the daemon when it is not running.
- The desktop shell needs its own copy of the binary. Run `node apps/desktop/build.mjs --with-daemon` first.
- A daemon from an older build can still be running. Stop it with `vellum daemon stop`, then run the command again. Check
  for it with `ps` if the stop does not take effect.

**Symptom:** the daemon fails to start with a message about the run directory or a socket.

The run directory must be private to your user, mode `0700`. Unix socket paths are limited to about 100 bytes, so a
long `--run-dir` path can fail even when the permissions are right. Choose a short path under your home folder for
isolated runs.

## An agent does not see the tools

**Symptom:** the client starts but lists no `vellum` tools, or lists them and then reports the server as disconnected.

- Check the path in the client's configuration. It must be the full path to the binary, because clients do not inherit
  your `PATH`.
- Restart the client after you change its configuration. Most clients read the file once at start-up.
- Run `vellum mcp` in a terminal and check that it stays open and waits for input. It should print nothing until a
  client sends a request. An error on start-up is the cause.
- Check the connection from the command line with `vellum tool get_app_state --json`. If that works, the server is
  fine and the problem is in the client's entry.

## The editor shows the file as read-only

**Symptom:** the VS Code editor opens a `.vellum` file and you cannot change it.

A file opens read-only when the daemon cannot hold it. The usual causes are these:

- The file is outside the folder that VS Code has open. Open the folder that contains the design.
- The daemon is not running, or cannot be reached. Run `vellum status --json`.
- The file cannot be read as a document. Open it as text, which the editor offers, and check it for a mistake.

## A permission was refused

**Symptom:** a tool fails with a message about permission, or a path or a stock search is refused.

Reading a file in the repository, or a search of the stock library, needs your permission because it reaches outside the
canvas. The daemon does not have a permission prompt yet, so those two calls fail there with a message that suggests
passing the content itself as `data` or `text`. The [permissions and safety](/docs/using-vellum/permissions-and-safety)
page covers what needs a permission.

## Browser capture fails

**Symptom:** a page capture or an element import fails with a message about a browser.

Set `VELLUM_CHROME` to the path of an installed Chromium, or run the capture from the desktop app, which has its own
Chromium. The capture does not use your personal browser profile.

## A key is not found

**Symptom:** a provider or a stock search says it has no key.

Keys live in the operating system's keychain under the service `vellum`. Store one with the asset key command, or with
the provider's own setup. In CI, a key comes from an environment variable named for its provider, such as
`VELLUM_OPENROUTER_API_KEY`, and only when `CI` is set. The [providers and keys](/docs/reference/providers-and-keys)
reference lists every name.

## Still stuck

Run the failing command with `--json` and keep the output. The error names the file, the field or the line that caused
it. The [verification](/docs/development/verification) page lists the checks that prove each part works, so you can
find which part is failing.
