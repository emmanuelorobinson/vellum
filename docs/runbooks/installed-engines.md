# Installed-engine tests and recordings

Claude Code is Prompt 36 (ADR-007); Codex is Prompt 37. There is no new editor UI or `vellum prompt` command
in this prompt. A host constructs `installed_engines::claude`, configures its `Session`/turn and supplies
`RegistryBridge`, then records the same steps and TurnEvents as the built-in engine.

The adapter checks the installed version's help and `auth status --json`. It never logs in, reads credential
files or prints auth output. Supported installed flags were verified on Claude Code 2.1.301. A sandbox may
hide the OS keychain and report signed out even when native execution is signed in; run the engines lane
with the same native access as the host. Ordinary user/project Claude settings and native tools are disabled;
Vellum context and skills are supplied through its own request and MCP tools. Managed policy still applies.

## Default replay lane

```
cargo test -p vellum-engine-claude-code
```

Replays need no installed engine, sign-in or network. `.synthetic.ndjson` is synthetic TEST DATA;
`.recorded.ndjson` is a projection captured from the named real version. Both are replayed as protocol input.
The version directory is not a promise of support for versions whose required flags fail the help probe.

## Local native boundary lane

```
cargo test -p vellum-engine-claude-code --test native -- --ignored
cargo test -p vellum --no-default-features --test installed_engine turn_mcp_relay -- --ignored
```

The first uses the Python 3 fixture on Unix, not a provider. It verifies output bounds, truncated framing,
diagnostic suppression, and Stop/drop killing the child tree within 3 s. The second runs the real Vellum MCP
binary over a private socket/pipe and forwards a request through the real registry bridge. It needs IPC
outside restrictive sandboxes. The existing three-platform Native IPC workflow runs it; the Python process
checks run on Unix only. The Windows lifecycle uses Job Objects; native Windows execution remains required.

Runtime paths must fit the platform's Unix socket length limit (roughly 100 bytes including the random
turn suffix and socket filename). Use a short private runtime root; tests use a freshly created private
folder under `/tmp` on Unix. Endpoints, discovery files and per-turn folders are removed when the turn ends.

## Real engines lane

Uses your existing installed sign-in and model quota. It never updates or signs into Claude Code.

```
cargo test -p vellum --no-default-features --test installed_engine engines_lane -- --ignored --test-threads=1
bash scripts/record-engine-fixtures.sh --offline
```

Optional `VELLUM_CLAUDE_CODE` selects an executable for the tests. Recorder options:
`--claude <path>`, `--model <alias>`, `--out <folder>`, `--case success|signed-out`. `--help` performs no build
or engine call. The signed-out case requires an actually signed-out installation; it never logs out an account.

The recorder uses the production launcher, private MCP endpoint and actual registry with a fixed read-only
TEST DATA prompt. It validates completion and at least one successful MCP call before writing the success
fixture. Files are grouped by actual installed version with a manifest; raw native output is never saved.
No arbitrary repo prompt or attachment is recorded. Review the projected fixture before committing it.
The negative case expects the explicit authentication status, not any generic failure.

Nightly: set repository variable `VELLUM_ENGINES_ENABLED=true` and provide a trusted self-hosted runner with
label `vellum-engines`, the pinned toolchain, Node 24, and the person's existing Claude Code sign-in. The
workflow runs only on schedule/manual dispatch, never on pull-request code. It runs the live MCP/usage and
Stop tests and uploads the projected versioned transcript as an artifact. No subscription credentials are
stored in this repository or in GitHub Actions secrets by this workflow.
