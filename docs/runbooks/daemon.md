# Local daemon and IPC lane

Start the foreground daemon with `cargo run -p vellum -- daemon`. Its default runtime directory is
`~/.vellum/run/`. `vellum status` authenticates without starting it. `vellum daemon stop` requests a
clean stop. `vellum mcp` starts/reuses the daemon and bridges stdio; stdout is exclusively MCP traffic.
The tool registry arrives in Prompt 29; currently requests receive an explicit method-not-found error.
No account, model, network access or renderer is needed for this transport.

For isolated development use `vellum --run-dir /path/to/private/run daemon` and give the same
`--run-dir` to `status`, `daemon stop` and `mcp`. The immediate parent must exist and be trusted.
The run directory is created privately. Existing directories and files must already be owned by
the current user with mode 0700/0600 on Unix or an equivalent protected user-only Windows DACL.
Symlinks/reparse points and nonprivate paths are errors. Do not delete lock files while processes
are running. Do not print/copy `daemon.json`: it holds the local token. API/provider credentials
belong in the OS keychain and never in this file.

Hosts can call the adapter's `ensure_daemon` with the bundled executable and runtime directory
after a disconnect. Startup is serialized and bounded; crashed processes release their lock.
Reconnecting does not replay requests. A protocol mismatch or authentication failure needs a
restart/upgrade or a corrected private directory, not automatic replacement of the daemon.
The editor/desktop host implementations arrive in later prompts.

TCP is opt-in, for a configured container/tunnel:

```sh
vellum daemon --listen 127.0.0.1:8000 --allow-origin https://editor.example
```

Use `POST /rpc`, exact `Host: 127.0.0.1:8000`, `Authorization: Bearer <local token>` and a bounded
Content-Length JSON-RPC body. Browser Origin must exactly match one of the repeated allow-origin
arguments. Native clients can omit Origin. `null` Origin is refused. There is no CORS or TLS;
this endpoint must be carried through the person's trusted tunnel. Bind with port 0 to choose a
free port; authenticated status reports the selected address. The local socket stays available.

Run the IPC lane (requires permission to bind local sockets and loopback TCP):

```sh
node scripts/ipc.mjs --offline
cargo test --offline -p vellum-ipc-local --lib
cargo clippy --offline -p vellum-ipc-local -p vellum --lib --bins -- -D warnings
pnpm checksum
```

Remove `--offline` when dependencies are not cached. The script builds the daemon binary and runs
all tagged process tests using `VELLUM_IPC_BINARY`; it does not compile renderer dev-dependencies.
Inventory checks require `lsof` on macOS, `/proc` on Linux, and PowerShell/Get-NetTCPConnection on
Windows. `.github/workflows/ipc.yml` runs all three platforms. Ordinary workspace tests show the
process cases as explicitly ignored; the IPC lane executes them. Windows DACL tests inspect actual
security descriptors; a cross-target compile alone is insufficient acceptance.

The local hello/welcome, bounds and lifecycle decisions are documented in ADR-003. No design
files, documents or session stores are opened by this transport. Existing API-key canary scanning
must never treat the separate local rendezvous token as a provider credential.
