# ADR-007: Installed engines

Status: proposed; Claude Code section implemented in Prompt 36. Codex section belongs to Prompt 37.
ARCHITECTURE has not been edited by this prompt.

## Claude Code

The `vellum-engine-claude-code` adapter implements the existing `AgentEngine` contract. The composition
root supplies a `McpHost`; the adapter supplies a `Launcher` (native process or transcript replay).
`availability` reads cached status. Explicit `recheck` probes `--version`, `--help`, and `auth status --json`;
only the version and logged-in boolean survive. Missing, signed out, failed/incompatible and usage-limited
are distinct. Probes have a five-second deadline and a 128 KiB output ceiling. No install, update or sign-in
is attempted. The version verified locally is 2.1.301; required flags are checked in that version's help.

Every turn gets a private endpoint in a fresh user-only runtime folder, using the existing IPC adapter's
Unix socket / Windows named pipe, discovery authentication, and frame bounds. Claude's explicit MCP config
launches the actual `vellum --run-dir <turn folder> mcp`. This endpoint answers discovery and tool schemas;
`tools/call` crosses a bounded mailbox to the turn's borrowed `ToolBridge` on the document thread. Thus it
uses the existing session, actor, turn, Guard and step recorder instead of manufacturing an external MCP
session with unrelated origins. Screenshot content uses the ordinary MCP image encoder. No TCP listener.
The composition root can reuse this transport for Codex without making adapters import one another.

Run one headless process per turn: print, stream-json input/output, partial messages, verbose protocol,
explicit model/effort, strict MCP config, no session persistence. Requests and pictures go over stdin;
prior portable history is explicitly quoted context, with markers for prior pictures/opaque continuations.
This is not native Claude resume: Vellum's session store remains the conversation owner. The CLI's
`--max-turns` and output-token environment setting carry the corresponding turn bounds; the adapter bounds
failed bridge calls. Text tool results use the shared character limit with an explicit truncation marker;
pictures travel separately as MCP image blocks and history keeps a marker instead of another base64 copy. Every record is at most 4 MiB and the process-to-engine queue holds 32 records.
EOF without a result, malformed records and process failures never become success. Unknown lifecycle
records are ignored, never interpreted as completion. Native diagnostic/error text is not forwarded.

Canvas-only scope: native tools, slash commands, auto memory, ordinary settings and custom hooks are disabled;
permissions are `dontAsk` with no unattended prompt handler. Only Vellum MCP tools are pre-approved.
There is no skip-permissions default, no native shell/file executor, and no engine-auto bypass. Future native
repo actions require a broker integration and an update to this ADR; they must not be enabled by simply
changing these flags. Managed Claude policy still applies. The installed tool retains its own authentication
and provider environment; Vellum never consults its SecretStore for it. A small OS/provider allowlist removes
all VELLUM_* values and the nested-session sentinel. Telemetry/error-reporting environment opt-outs are set.
This isolation also means ordinary user/project Claude settings, plugins and customizations are not reused.

The bridge's actual calls emit started/finished events exactly once; Claude's duplicate observed MCP tool
records do not execute tools or create duplicate steps. A start is delivered before execution, so Stop at
that boundary prevents the call. Assistant partials do not duplicate the final assistant text. Final usage
normalizes Anthropic's cache-exclusive input into inclusive input, preserving cache reads and writes;
subscription reset time is retained only when explicitly reported, converted from seconds to UTC ms.
No subscription percentage or cost is invented. Sign-in and rejected rate-limit events update engine status;
a usage-limit failure retains a reported reset time until recheck. The later UI uses existing configured copy.

Stop/drop cancels every process wait, discards pending MCP work, terminates the owned process group (Unix)
or kill-on-close Job Object (Windows), and closes the turn endpoint. No graceful signal is required before
the hard stop; the adapter owns the entire group and all completed canvas work stays. Process termination
is also used after completion to avoid orphaned MCP relays. The Guard's session Stop remains Prompt 39.
Windows job attachment and native Windows/Linux execution need their platform lanes; local evidence is macOS.

## Reference note

Goose's `crates/goose/src/providers/claude_code.rs` uses persistent stream-json subprocesses, an explicit MCP
config, partial text and cache-inclusive usage, and a stdio permission-control callback. Vellum adapts the
launch/projection/usage portions with an Apache-2.0 header, notice row and bundled license. Vellum changes
process reuse to one process per turn and routes MCP back to the borrowed bridge to preserve origins.
Native permission callbacks and Goose's default skip-permissions behavior are not ported. The sibling
Outpost implementation was read for bounded process-tree cleanup and typed diagnostic projection; no code
was copied from it.

## Verification sources and fixtures

Installed `claude --version`, `claude --help`, `claude auth status --help`, plus current official docs:
[headless](https://code.claude.com/docs/en/headless), [CLI reference](https://code.claude.com/docs/en/cli-reference),
[MCP](https://code.claude.com/docs/en/mcp), and settings via Context7. The flags and login checks are verified
against the executable, not inferred from Goose's older flags. `--bare` is deliberately absent because it
disables subscription/keychain auth.

`fixtures/engines/claude-code/<installed version>/` separates `.synthetic.ndjson` from `.recorded.ndjson`.
The recorder uses a fixed TEST DATA prompt, no attachments, a read-only get_app_state call and a 90 s watchdog.
Only decoder fields are saved: no raw stderr, auth metadata, cwd/session identifiers, signatures or native
error text. A manifest records the real version and case. Default tests replay fixtures without an engine,
account or network. The `engines` nightly workflow is opt-in on a trusted signed-in runner, never a PR runner.

## Codex

Not implemented here. Prompt 37 evaluates app-server first and exec only when app-server is unavailable.
