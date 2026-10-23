---
title: "The daemon"
description: "The local process that owns documents: how it runs in live and headless modes, how it protects its connection, runs the tools, orchestrates agents, and records sessions."
---

The daemon is a native program on your computer. It owns the documents you have open, runs the tools that change them, starts the agent
turns, and writes the files. The command line and the MCP server are the same program, so there is one daemon per private folder.

## Live and headless

A document is either held by an editor or held by the daemon.

- **Held by an editor (live).** The first editor that opens a file becomes its writer. The daemon forwards changes to that editor, so the canvas
  updates as agents work. Later editors attach and follow every change, including unsaved ones.
- **Held by the daemon (headless).** With no editor open, the daemon loads the file into its own engine, applies each change through the
  checks, renders when asked, and saves after every change. A failed save keeps the change and retries on the next save.

Agents work in both modes. An agent can design with no app open, in continuous integration, or on a server.

When the writer closes, the next editor is promoted, and when no editor is left, the next tool call opens the file headless again. A writer
that stops answering is dropped. Without the daemon, an editor cannot know who is writing, so it opens the file read-only.

## Recovery copies

For a live file with unsaved changes, the daemon writes a recovery copy to `~/.vellum/recovery` at most every five seconds. A save removes the
copy. A writer that stops answering keeps it, so the next open can restore the work.

## Local connection security

The daemon listens on a Unix socket in its private folder, which only your user can read. On Windows it uses a named pipe with an access list for
the current user only. Nothing else can reach it, and no web page in a browser can, because the daemon has no network port unless you open one.

Only one daemon runs for each private folder. A lock file serialises starts, and a second start reuses the running daemon. There is no process
killing: a crashed daemon releases its lock.

Each connection sends a bounded opening message with a token from the private folder, which is a fresh 256-bit value each time the daemon starts.
The daemon checks the token before it negotiates the protocol version. It allows at most 64 sessions, gives a handshake three seconds, and caps a
frame at one mebibyte.

A TCP listener exists only when you start the daemon with `--listen`. It takes one bounded request per connection, with a bearer token, an exact
host name, and an allowed origin when a browser sends one. There are no cookies and no sockets over it. The [running the daemon](/docs/operations/running-the-daemon)
page covers the operating side.

## The tools

Every tool is one entry in the tool registry. The same registry serves the MCP server, the command line and the built-in agent, so a tool behaves
the same whichever front end calls it. A tool reaches documents only through a context the daemon provides, which routes to the live editor or the
headless engine. A tool does not know which one it is talking to.

A tool's name is stable. Its schema may gain fields, and a breaking change gets a new name.

## Agents and sessions

Every agent gets an identity: a name, a colour and an identifier. Each MCP connection is one agent, named after the client that opened it. Every change
the agent makes carries that identity.

A claim holds a frame for one agent. A plan turns a multi-frame request into reserved areas. Parallel lanes run as their own agents, and pause and stop
are checked on every change.

Sessions are recorded for each file, in an append-only log. A session holds the conversation, the steps, the usage and the state of each turn. The
store refuses an event from another session, and it keeps thumbnails out of the database, in the cache.

## Permissions

Changes to the canvas never need permission. Anything outside the canvas goes through the permission broker, which asks a person through a card in the
agent panel, or refuses. Without a host to ask, a request is refused and recorded. The [permissions and safety](/docs/using-vellum/permissions-and-safety)
page describes the defaults.
