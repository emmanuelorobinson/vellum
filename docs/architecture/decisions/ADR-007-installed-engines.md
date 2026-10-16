# ADR-007: Installed engines

Status: proposed for owner review; Claude Code implemented in Prompt 36, Codex in Prompt 37.
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

`vellum-engine-codex` implements the same `AgentEngine` and portable-history contract. Explicit bounded
probes read `--version`, root/app-server/exec help and `login status`. The retained installation contains
only version, status and transport. ChatGPT and existing API-key sign-in are recognized; API-key suffixes,
account names and raw diagnostics are discarded. Availability is cached; Re-check repeats the probes.
The locally verified executable is 0.158.0, older than the reference pin. `SUPPORTED_VERSIONS` rejects
unrecorded versions until their flags, protocol, tool isolation and native lifecycle have been checked.
No installation, account switch, login or update is attempted. Credentials remain with Codex; the adapter
never reads Vellum SecretStore or credential files.

Prefer one owned `codex app-server --listen stdio://` process per turn. Sequence initialize/initialized,
config/read, thread/start and turn/start by correlated request IDs. Create an ephemeral thread with empty
environments, explicit model/effort, portable quoted history, optional data-URL images and the registry's
schemas as dynamic tools. App-server `item/tool/call` requests go directly to the borrowed ToolBridge.
Only declared names, object arguments, the active thread/turn, and unique call IDs are accepted. Start is
emitted before executing, so Stop at that boundary prevents the call. Observed tool lifecycle events
never execute another call or create another registry step. Tool replies preserve images as separate
inputImage content, bound text explicitly, and keep picture markers in portable history.

This differs from Claude's MCP attachment deliberately: the initial MCP trial completed without a tool
call. App-server provides direct tools, so this path avoids an extra MCP subprocess while using the same registry,
Guard, session and origin. The installed models require Codex's sandboxed code-mode host to expose these
tools; disabling that host produced completed replies without tool calls in live verification. The host
is enabled, while shell, stable environment tools, file-image tools, hooks, plugins, apps, browser/computer
use, image generation, memories, remote control, native subagents and goals are disabled. Web search is
disabled, native approval policy is never, and unexpected native requests fail closed. User MCP server
entries are explicitly disabled per thread before starting work. No native repository executor or engine
auto mode is enabled; those still require PermissionBroker integration. Managed policy remains effective.

Use `codex exec` only when the help probe says app-server is absent. Failure, malformed output, auth or
quota rejection after choosing app-server never retries through exec (which could repeat edits). Exec
uses JSONL, ephemeral mode, ignores user configuration/rules, takes the prompt over stdin, and attaches
only the fresh private `vellum mcp` relay from Prompt 36. Explicit configuration disables native tools and
telemetry; the sandboxed code-mode host remains available. Exec image attachments are refused explicitly
because stdin does not support them; no silent image loss or persisted temporary picture files. The
recorder's explicit `--transport exec` exercises fallback without pretending the installed CLI lacks
app-server. This override is a test harness, not automatic production failover.

Assistant deltas/final items are deduplicated per item. App-server cumulative token usage is emitted once
at the terminal event, without adding cached input again; exec completion usage has the same inclusive
normalization. Cache writes and reasoning are retained when reported. Reported primary/secondary quota
windows are separate from token usage; reset times are UTC milliseconds and are retained only when the
native engine supplies them. Structured auth/quota errors become SignedOut/UsageLimit; exec's limited
error shape uses only known diagnostic prefixes and never forwards native text. Unknown errors are
Failed, never guessed sign-out or success. No subscription cost or percentage is invented.

Each native record is capped at 4 MiB; both directions have 32-record queues. RPC writes are capped at
8 MiB, tool text uses TurnLimits, app-server cumulative output/round usage is bounded. Decoded assistant text is also capped at 32 bytes per
requested output token before usage arrives, and each turn accepts at most 10,000 native records. The exec protocol
reports model usage only at completion, so round/output limits cannot stop an in-flight request from
usage feedback; Stop remains available at every wait. Its failed canvas calls are bounded on both paths.
EOF without a terminal result, incomplete framing, wrong correlation and unknown terminal statuses fail.
Stop/drop cancel waits and terminate the owned Unix group / Windows Job Object, including MCP relays;
portable Vellum history remains the conversation owner. App-server's turn/interrupt RPC is not required
because the entire ephemeral process belongs to this one turn and is terminated directly.

### Codex reference note

Goose's `providers/codex.rs` runs one exec process per turn, sends text on stdin, configures MCP with TOML
overrides and normalizes inclusive cached usage. Vellum adapts these portions with the source header and
notice; it does not port Goose's skip-permissions defaults, debug output, buffered conversation or image
files. `codex_acp.rs` delegates to a separate ACP binary; Vellum keeps the installed Codex executable and
uses its documented app-server schemas instead. The pinned Codex README/protocol and exec/config docs,
informed protocol choices. Sibling Outpost paths were checked for Codex integration; Vellum’s existing
Claude process-tree lifecycle was reused.

### Codex verification and fixtures

Official [app-server](https://developers.openai.com/codex/app-server) and
[non-interactive mode](https://developers.openai.com/codex/noninteractive) documentation, Context7's
`/openai/codex` reference, installed help and generated experimental JSON schemas were checked. The local
catalog includes `gpt-6-luna`, used for the fixed read-only fixture and live acceptance. Hosts still supply
the requested model; the adapter never substitutes another model.

`fixtures/engines/codex/0.158.0/` separates synthetic from real projected app-server/exec transcripts.
Configuration/auth replies, native identity, stderr, reasoning text and diagnostics are excluded. Only
safe item IDs, fixed TEST DATA assistant text, numeric usage and a read-only get_app_state request are
retained. Negative auth/quota cases are synthetic; no account was logged out or quota exhausted. Default
replays and shared contracts require no engine/account/network. The native Python fixture tests both
pipes, framing, output limits and Stop/drop; the trusted nightly lane runs live canvas/usage and Stop,
then records both transports. macOS arm64 is the local native evidence; Windows/Linux require their lanes.
