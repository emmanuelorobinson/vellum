# ADR-000: Deltas D1–D6 from ARCHITECTURE.md

- **Status:** Proposed. Every delta below is a proposal until the prompt that confirms it accepts or rejects it.
- **Date:** 2026-10-11
- **Written by:** Prompt 2 (guide §6.1 and §2)
- **Source:** `docs/implementation/vellum-implementation-guide.md` §6.1. The research behind each delta is in the
  reference repositories pinned in `scripts/refs.tsv` and indexed in `docs/reference/REFERENCE-INDEX.md`.
- **Scope:** `docs/architecture/ARCHITECTURE.md` is not edited here. Once a delta is confirmed, a person updates
  that document (guide §2: `docs/architecture/` changes only through ADRs and the version baseline).

| Delta | Decision (proposed) | Confirmed or rejected by | Writes |
|---|---|---|---|
| D1 | One Rust renderer everywhere: skia-safe compiled to WASM | Prompt 4 | ADR-001 |
| D2 | Layout goldens come from headless Chrome | Prompt 6 | ADR-002 |
| D3 | `vellum mcp` is a stdio-to-socket bridge | Prompt 27 | ADR-003 |
| D4 | Installed engines ported from Goose adapters; Codex through its app-server protocol | Prompts 36–37 | ADR-007 |
| D5 | `execute` follows the code-mode pattern | Prompts 25–26 | ADR-005 |
| D6 | Session storage follows rollout-style append-only logs | Prompt 34 | ADR-006 |

## D1 — One Rust renderer everywhere: skia-safe compiled to WASM

Status: Proposed
- **Confirmed by:** Prompt 4 (writes ADR-001)

**Context.** ARCHITECTURE §3.2 has the editor drawing with CanvasKit from TypeScript and the daemon drawing with
skia-safe natively, the two matched by goldens. Penpot's `render-wasm` already ships Rust and Skia compiled to WASM
(emscripten), with tile rendering, text and fills in Rust (`refs/penpot/render-wasm`, guide §3.2 R1).

**Decision (proposed).** Use skia-safe compiled to WASM as the one renderer for the editor and the headless path. Then
the editor and the headless renderer are the same code, the text-parity question goes away, and Invariant 6 is nearly
free. The `Renderer` port stays either way, and the spike in Prompt 4 decides the default.

**Alternatives.** CanvasKit drawn from TypeScript for the editor, with skia-safe native in the daemon (ARCHITECTURE §3.2).

**Costs.** The emscripten toolchain, a larger WASM bundle, and GPU context handling in Rust.

**Revisit when.** The Prompt 4 spike shows the WASM path cannot match the CanvasKit path within the golden tolerance,
or the emscripten target cannot be installed for the pinned toolchain in CI (`rust-toolchain.toml` already lists
`wasm32-unknown-emscripten`).

## D2 — Layout goldens come from Chrome, not from hand-written expectations

Status: Proposed
- **Confirmed by:** Prompt 6 (writes ADR-002)

**Context.** Taffy generates its test fixtures by laying out HTML in headless Chrome (`refs/taffy/scripts/gentest`,
guide §3.2 R3).

**Decision (proposed).** Vellum uses the same oracle for its auto-layout semantics (Invariant 7): row, column, grid,
hug and fill, gap, padding, justify and align, and absolute children. Each fixture renders the CSS equivalent of a
Vellum frame in headless Chrome, and the layout must agree within 0.5 px.

**Alternatives.** Hand-written layout expectations, checked by reading the code.

**Revisit when.** A browser behaviour that matters to designs cannot be captured by rendering its CSS equivalent in
headless Chrome, or the Chrome oracle stops being reproducible in CI.

## D3 — `vellum mcp` is a stdio-to-socket bridge

Status: Proposed
- **Confirmed by:** Prompt 27 (writes ADR-003)

**Context.** Codex ships exactly this shape (`refs/codex/codex-rs/stdio-to-uds`, guide §3.2 R12). An MCP client launches
a small stdio process, and that process forwards to the user-only daemon socket.

**Decision (proposed).** `vellum mcp` does the same. Every client gets the live document when the editor has it open,
and the headless engine otherwise. There is no TCP port (Invariant 9).

**Alternatives.** MCP over streamable HTTP on localhost. ARCHITECTURE §1 names it, but it needs a network listener.

**Revisit when.** An MCP client we must support cannot launch a stdio server, or the bridge cannot run reliably over
Windows named pipes.

## D4 — Installed engines are ported from Goose adapters, and Codex is driven through its app-server protocol

Status: Proposed
- **Confirmed by:** Prompts 36–37 (writes ADR-007)

**Context.** Goose (Apache-2.0) already implements Claude Code, Codex, Cursor Agent and Gemini CLI as providers
(`refs/goose/crates/goose/src/providers`, guide §3.2 R14). For Codex, the JSON-RPC app-server protocol
(`refs/codex/codex-rs/app-server-protocol`) is preferable to parsing the output of `codex exec`.

**Decision (proposed).** Port the Goose adapters with attribution (a `ported-from:` header and a
`THIRD_PARTY_NOTICES.md` row) rather than reverse-engineering the CLIs. For Codex, use the app-server protocol, and fall
back to `exec` only when the installed version lacks it.

**Alternatives.** Write the adapters from each CLI's documentation alone. Parse `codex exec` output.

**Revisit when.** A tested Claude Code or Codex release changes its headless protocol in a way the ported adapter
cannot follow, or Goose's licence stops being Apache-2.0.

## D5 — `execute` follows the code-mode pattern

Status: Proposed
- **Confirmed by:** Prompts 25–26 (writes ADR-005)

**Context.** Codex and OpenCode ship code-mode runtimes, where a model writes a program against a typed API rather than
making many tool calls (`refs/codex/codex-rs/code-mode`, `refs/opencode/packages/codemode`, guide §3.2 R9). Penpot's MCP
server exposes code execution against its plugin API (guide §3.2 R8). This confirms the two-layer design in
ARCHITECTURE §7.3.

**Decision (proposed).** The runtime stays QuickJS: it is deterministic and embeddable in WASM (Invariant 8). The `doc`
API ships a generated `.d.ts`, so models get types, as those projects do.

**Alternatives.** Typed tool calls only. A sandbox that cannot run in WASM.

**Revisit when.** QuickJS in WASM misses the sandbox, determinism or size budgets, or the escape corpus finds an escape
that cannot be closed (ARCHITECTURE §15, question 5).

## D6 — Session storage follows rollout-style append-only logs

Status: Proposed
- **Confirmed by:** Prompt 34 (writes ADR-006)

**Context.** Codex's rollout and thread store, and Goose's session manager (`refs/codex/codex-rs/rollout`,
`refs/goose/crates/goose/src/session`, guide §3.2 R16), confirm the append-only event log per session described in
ARCHITECTURE §7.6.

**Decision (proposed).** SQLite holds the session index and the event log, with thumbnails in a content-addressed
cache.

**Alternatives.** One JSON file per session. No event log.

**Revisit when.** Listing or resuming sessions misses the performance budget on realistic histories, or the bundled
SQLite cannot ship on a target platform.
