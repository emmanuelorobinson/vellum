# Vellum

Design files and agent access in your repo. Local features require no account. The extension starts a bundled
native daemon over a private local socket; it does not open a network listener or enable telemetry.

**Development preview:** the document runtime is incomplete. Design files open read-only, and document contents
are not rendered yet. Use **Vellum: Toggle Design / Text View** to inspect the source. This package is not ready
for a public release.

## Agent setup

Run **Vellum: Connect Agents**, or choose **Connect agents…** from the canvas Agents menu. Review the client and
configuration path, then Connect. The dialog shows exactly the entry written. Restart the client to load it.
Configured means the entry exists; Connected means a matching client has initialized an active daemon connection.
Manual setup offers each client's snippet and Copy config. Disconnect removes only the matching Vellum entry;
existing client processes may remain connected until they exit. Modified or conflicting entries are left alone.

The extension supports Claude Code, Codex (desktop and IDE), Cursor, Windsurf, VS Code Copilot, Gemini CLI,
Antigravity, OpenCode, Kiro and Claude Desktop. Client versions can differ; see the configuration-path and schema
baseline in the repository's `docs/runbooks/connect-agents.md`. No client is executed during detection.

The Vellum Activity Bar view lists recent design files and recorded sessions for those files in the current
workspace. Session rows open their document; conversation navigation needs the unfinished editor session feed.
Refresh retries after an offline daemon. No sessions are fabricated.

Requires a trusted, local workspace and VS Code 1.140 or later. Cursor and Windsurf can install the matching
platform VSIX from Open VSX when they support that VS Code API baseline. Remote extension hosts configure clients
on the remote computer; desktop clients on another computer need manual setup there.
