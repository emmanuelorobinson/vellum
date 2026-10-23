---
title: "Connect clients"
description: "The configuration file and entry that connect each supported MCP client to {{brand}}, and how the extension writes them for you."
---

Every client connects the same way: it starts the `vellum` command with the argument `mcp`, and talks to it over standard
input and output. The client's configuration file tells it where the command is. The extension can write these files for you,
and this page lists where each one lives so you can check, or write one by hand.

Use the full path to the binary. Clients do not inherit your shell's `PATH`, and the extension uses its own stable copy of the
daemon, so a path that is correct in one terminal may not be found by a client.

## Supported clients

| Client | Configuration file | Entry |
| --- | --- | --- |
| Claude Code | `~/.claude.json` | `mcpServers.vellum`, with `type` set to `stdio`, `command` and `args` |
| Codex (desktop and IDE) | `$CODEX_HOME/config.toml`, or `~/.codex/config.toml` | `[mcp_servers.vellum]`, with `command` and `args` |
| Cursor | `~/.cursor/mcp.json` | `mcpServers.vellum`, with `command` and `args` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json`, or the Devin path where that is in use | `mcpServers.vellum`, with `command` and `args` |
| VS Code (Copilot) | `.vscode/mcp.json` in the workspace, or your user MCP file when no folder is open | `servers.vellum`, with `type` set to `stdio`, `command` and `args` |
| Gemini CLI | `~/.gemini/settings.json` | `mcpServers.vellum`, with `command` and `args` |
| Antigravity | `~/.gemini/antigravity/mcp_config.json`, or the newer path where that is in use | `mcpServers.vellum`, with `command` and `args` |
| OpenCode | `opencode.jsonc` or `opencode.json` under `$XDG_CONFIG_HOME/opencode` | `mcp.servers.vellum`, with `type` set to `local`, `command` as an array, and `enabled` |
| Kiro | `~/.kiro/settings/mcp.json` | `mcpServers.vellum`, with `command` and `args` |
| Claude Desktop | macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`. Windows: `%APPDATA%/Claude/claude_desktop_config.json` | `mcpServers.vellum`, with `command` and `args` |

`XDG_CONFIG_HOME` defaults to `~/.config`. `CLAUDE_CONFIG_DIR` moves Claude Code's directory. Codex uses one configuration for
its command line, IDE and desktop surfaces. Claude Desktop on Linux has no verified configuration path, so the extension offers
its usual candidate for you to review.

## What the extension writes

From VS Code, **{{brand}}: Connect Agents** detects the clients installed on your computer. It shows the entry it would write and the
file it goes into, and it writes nothing until you press the client's button.

- **Configured** means the entry is in the file. The client may need a restart before it shows the tools.
- **Connected** means the daemon has seen that client connect.
- **Disconnect** removes the entry only if it still points at the extension's daemon. An entry you have edited yourself is kept,
  and the dialog reports the difference.

The extension's daemon copy lives in the extension's own storage, and it contains no keys and no tokens.

## Writing an entry by hand

For Claude Code, add the entry to `~/.claude.json`:

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

Keep the other entries in the file. Write the change while the client is closed, and restart it afterwards.

## Checking the connection

Run `vellum tool get_app_state --json` from a terminal. If that works, the server is fine, and any problem is in the client's
entry or in the client itself. The [troubleshooting](/docs/getting-started/troubleshooting) page covers the common causes.
