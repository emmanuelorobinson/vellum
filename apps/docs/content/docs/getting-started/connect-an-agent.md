---
title: "Connect an agent"
description: "Register the {{brand}} MCP server with Claude Code, Codex, Cursor and the other supported clients, then check the connection."
---

An agent works on a document through the MCP server. The server is the `vellum` command with the `mcp` argument. A
client starts it, talks to it over stdin and stdout, and sees the same tool list the command line shows. Connecting an
agent means telling its client where that command is.

## The quick way: from VS Code

With the extension installed, run **{{brand}}: Connect Agents** from the command palette, or use the Agents menu on the
canvas. The dialog lists the clients it found on your computer and what it would write for each one.

- Nothing is written until you click a client's button. The dialog shows the entry and the file it goes into.
- **Connected** means the daemon has seen that client connect. **Configured** means the entry is in the client's file,
  and the client may need a restart before it shows the tools.
- Disconnect removes only an entry that points at this extension. An entry you edited yourself is kept, and the dialog
  reports the conflict.

The [VS Code extension](/docs/operations/vscode-extension) page explains the dialog in more detail.

## The manual way

Register the built command as a stdio server. Use the full path to the binary you built in
[install](/docs/getting-started/install), because a client does not inherit your shell's `PATH`.

Claude Code, in `~/.claude.json`, under `mcpServers`:

```json
{
  "mcpServers": {
    "vellum": {
      "type": "stdio",
      "command": "/absolute/path/to/vellum/target/debug/vellum",
      "args": ["mcp"]
    }
  }
}
```

Cursor, in `~/.cursor/mcp.json`, under `mcpServers`, uses the same `command` and `args`:

```json
{
  "mcpServers": {
    "vellum": {
      "command": "/absolute/path/to/vellum/target/debug/vellum",
      "args": ["mcp"]
    }
  }
}
```

Codex reads `~/.codex/config.toml`, or the file in `$CODEX_HOME`:

```toml
[mcp_servers.vellum]
command = "/absolute/path/to/vellum/target/debug/vellum"
args = ["mcp"]
```

The [connect clients](/docs/agents/connect-clients) page lists the file and the entry shape for every supported client.

## Check the connection

Restart the client, then ask it for the app state, or run the same tool from the command line:

```sh
vellum tool get_app_state --json
```

The state lists the open documents, the selection, the frames other agents hold and the notes for agents. An empty
document list is normal before you open one. Ask the agent to open `design/app.vellum` if it does not open one by
itself.

## Ask for a screen

Start with a small brief that names the file and the frame:

> Open design/app.vellum. Read the design system guidelines first. Then design a three-step onboarding flow as three
> frames side by side on a new page called Onboarding, using the variables that already exist.

The agent reads the skills and styles it needs, claims the frame it works in, makes its changes in batches, checks the
layout, and releases the frame. The [working with the tools](/docs/agents/working-with-the-tools) page describes that
sequence.

## When it does not connect

See [troubleshooting](/docs/getting-started/troubleshooting). The usual causes are a relative path in the client's entry,
a client that was not restarted, and a daemon from an older build still running.
