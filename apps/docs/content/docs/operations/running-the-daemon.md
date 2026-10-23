---
title: "Running the daemon"
description: "The daemon owns your documents while agents and editors work on them. How it starts, where its private files are, how to stop it, and how to run an isolated copy."
---

The daemon is the local process that owns your documents. Agents, the command line and the editors all talk to it, so it is
the one place where a change is checked, attributed and written. It runs on your computer, and it needs no account, no model
and no network access to start.

## How it starts

Commands that work with documents start the daemon when it is not running. So do `vellum mcp`, which an agent client runs, and
the editor hosts. `vellum status --json` reports whether the daemon is running, without starting it.

To run the daemon in the foreground, for example to watch its output:

```sh
vellum daemon
```

## Where its files are

The daemon keeps its private files in `~/.vellum/run` by default. The folder and its files are readable only by your user. The
file `daemon.json` holds the local token that clients use to connect. Do not print it, copy it or put it in a file that is
committed. Provider keys are never in this folder; they live in the keychain.

## Stopping it

```sh
vellum daemon stop
```

A stop is a clean stop: the daemon completes what it is doing, then releases its lock and exits. A daemon that a crash left
behind holds no lock, so the next command starts a fresh one.

Do not delete the lock files while a daemon is running. The lock files tell a second daemon that the first one is alive.

## An isolated daemon

Give every command the same `--run-dir` to run a separate daemon with its own sessions and state. Use this for tests and for
continuous integration:

```sh
vellum --run-dir "$RUNNER_TEMP/vellum/run" status --json
vellum --run-dir "$RUNNER_TEMP/vellum/run" daemon stop --json
```

The folder you name must exist, and its parent must be a folder you trust. The daemon creates the run folder privately. A folder
that already exists must be owned by your user and private, and a symbolic link is refused. A socket path is limited to about 100
bytes on Unix, so keep the run folder path short.

## Remote access, carefully

The daemon listens only on a private local socket by default. It can also listen on a TCP port, but only when you ask for it:

```sh
vellum daemon --listen 127.0.0.1:8000 --allow-origin https://editor.example
```

A TCP client must send the local token as a bearer credential, and it must use the exact host and origin you allowed. There is no
TLS and no CORS on this endpoint, so put it behind a tunnel you trust, and never expose it directly to the internet.

## When versions disagree

A client and a daemon built from different versions refuse to talk. The fix is to restart the daemon with the build you want, not to
let the daemon replace itself. Stop the old one with `vellum daemon stop`, then run a command again.
