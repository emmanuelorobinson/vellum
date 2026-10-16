# ADR-003: private local daemon and stdio bridge

Status: implemented for Prompt 27; native Windows/Linux execution awaits the IPC CI lane.

## Decision

`vellum daemon` runs one foreground native process per runtime directory. The default directory is
`~/.vellum/run/`. Hosts and `vellum mcp` use `ensure_daemon` after a disconnect. They serialize startup
with `startup.lock`; the daemon holds `daemon.lock` for its entire lifetime. Both are OS file locks,
released after a crash. Startup waits for the old instance lock during graceful HTTP draining. Lock files are permanent and are never unlinked: a waiter must retain the
same inode as later starters. No PID killing or PID-based ownership tests are used.

Unix uses `vellum.sock`, a 0600 Unix socket in a 0700 directory. Windows uses
`\\.\pipe\vellum-<current-user-SID>-<run-directory-hash>`; the suffix isolates explicitly selected
runtime directories. Direct Win32 calls create a protected DACL with one full-access ACE for the
current user, on directories, files and every pipe instance. Pipes reject remote clients and the
first instance uses `FILE_FLAG_FIRST_PIPE_INSTANCE`. Existing directories/files must already have
the expected owner and private permissions/DACL. Symlinks and Windows reparse points are refused.
Unix lock/discovery files must have exactly one hard link. Unsafe existing paths are refused,
never silently repaired. Ancestors of an explicitly selected directory must be trusted by the host.

The implementation uses Tokio's native Unix socket and named-pipe APIs in `vellum-ipc-local` rather
than adding `interprocess`: Tokio supplies the necessary cancellation and pipe controls, and Win32
is still needed for the explicit security descriptor. All vendor/OS dependencies stay in the
adapter; the composition root only parses commands. `vellum-types::daemon` owns the v1 handshake.
The first protocol version supports only v1; future versions must retain their previous version.

## Discovery and authentication

After acquiring the instance lock and binding listeners, the daemon publishes a new 256-bit OS-random
local token in `daemon.json` (0600 / user-only DACL). It writes and syncs a private temporary file,
then renames it. Windows removes the old discovery file before rename under the instance lock;
readers retry across the gap. The endpoint must match the endpoint derived from the runtime directory.
The discovery record contains version, PID, endpoint, token and optional bound TCP address. It has
no Debug implementation, and status output omits the token. No token is passed in argv or logs.
This ephemeral local rendezvous token is the explicit §7.2 exception on disk; provider/API credentials
remain keychain-only and are never loaded by this adapter.

Each local connection sends a bounded JSON-line hello (`v`, `token`, `channel`: `mcp` or `control`).
Authentication precedes version negotiation; refusal reports only a fixed typed status. The bridge
consumes the welcome before touching MCP stdout. The channel is private transport routing, not
an agent identity or permission grant. Document/session identity and tools arrive in later prompts.

## Lifecycle and limits

The daemon accepts at most 64 concurrent local/HTTP sessions. Handshakes, control commands and HTTP
requests have a three-second deadline; local frames are capped at 1 MiB and hellos at 4 KiB. Slow
unauthenticated clients cannot create unbounded tasks. MCP sessions stay open until disconnect.
No request is replayed after failure. Startup has a ten-second deadline and the launching process
reaps the daemon child. An unavailable endpoint permits restart; authentication/protocol/security
failures are reported without replacing a live daemon.

`daemon stop`, SIGINT and Unix SIGTERM stop acceptance, cancel waiting local sessions, allow bounded
HTTP tasks to finish, and remove discovery and the Unix socket before releasing the lock. Crashes
leave discovery/socket paths behind; the next lock holder replaces stale sockets and rotates the
token. There are no daemon-owned documents, sessions or recovery copies yet (Prompt 28).

Private MCP framing uses newline-delimited JSON and a blank-line end marker. The bridge translates
stdin EOF into that marker; the daemon flushes all replies and returns its marker. The bridge strips
it from stdout. This supplies a portable half-close for named pipes. Abrupt EOF is an explicit
"daemon disconnected" error. The bridge gives runtime shutdown a bounded wait because Tokio's
blocking stdin read cannot be cancelled when a client keeps stdin open. This API is a process entry
point. On macOS, child launch marks inherited descriptors CLOEXEC in an async-signal-safe pre-exec
callback: concurrent host pipe creation can otherwise leave an unrelated pipe in the daemon.

## Explicit TCP

There is no default TCP bind. `--listen <IP:port>` opts in to one HTTP listener, alongside local IPC.
It accepts one bounded `POST /rpc HTTP/1.1` per connection with a Bearer token, an exact Host matching
the bound address, and an exact allowlisted Origin when present (`--allow-origin`, repeatable).
Absent Origin permits native clients; `null` is always refused. No wildcard origins, CORS, cookies,
websocket upgrade, chunking, duplicate headers or DNS Host names are accepted. Header bytes are
capped at 16 KiB, bodies at 1 MiB. This is plaintext transport for a person's explicitly configured
container/tunnel; it is not a public Internet endpoint. No TCP control/shutdown channel exists.

## Reference review

Read Codex R12 `stdio-to-uds/README.md`, `stdio-to-uds/src/lib.rs`, and `uds/src/lib.rs`.
Codex relays both directions concurrently and half-closes on stdin EOF; its UDS wrapper supplies
platform/security boundaries. Vellum keeps concurrent forwarding, moves authentication ahead of
stdout, supplies a named-pipe end marker, and starts/reuses one private daemon. The sibling Outpost
startup lock and private-socket patterns informed the design. No reference code was ported.

## Evidence and scope

The IPC lane checks simultaneous startup, reuse, graceful stop, signals, crash/token rotation,
private paths, authentication/version refusal, fragmented frames, stdin EOF, disconnect with stdin
open, actual listener inventory, and explicit TCP token/Host/Origin checks. Adapter unit tests cover
bounded/truncated framing, silent notifications, handshake refusals and unsafe discovery paths;
Windows additionally checks rejection of a broad DACL. `scripts/ipc.mjs` builds only the binary and
adapter, so this lane needs no renderer. GitHub runs it on macOS, Linux and Windows; only macOS arm64
has executed locally. Cross-target check/clippy covers Windows code but does not validate OS behavior.

MCP registry/initialize/tools are deliberately absent until Prompt 29: valid requests receive a
JSON-RPC method-not-found error stating this; notifications receive no response. This prompt supplies
the transport, not a usable design-tool server. Real VS Code/Electron restart wiring is later host work.
