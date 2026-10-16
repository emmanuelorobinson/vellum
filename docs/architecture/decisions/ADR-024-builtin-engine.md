# ADR-024: Built-in engine loop and context management

Status: proposed; implemented in Prompt 33. Only a person synchronizes ARCHITECTURE.md.

## Decision

`vellum-core::ports` gains `AgentEngine`, `ToolBridge`, `TurnRequest`, `TurnEvent` and `TurnStream`.
They differ from ARCHITECTURE §3.4 in shape, not intent:

- `run_turn(&self, TurnRequest, &dyn ToolBridge)` is synchronous and returns a `TurnStream` that borrows the
  bridge. The stream's futures are not `Send`: the bridge wraps the daemon's documents, which live on one
  thread (ADR-021). The engine itself is `Send + Sync`, so `interrupt(&TurnId)` works from any thread.
- Failures are a terminal event (`Ended(Failed(EngineError))`), not a stream error. Every turn ends with
  exactly one `Ended`, preceded by `Messages` (what the turn added, for the next turn's history).
- `ToolBridge` is two synchronous calls, `tools()` and `call(name, input)`, answering JSON plus a failed flag.
  `crates/vellum` implements it over the registry (`RegistryBridge`): each call is an ordinary tool run,
  recorded as a step, Guarded, and attributed to the session's agent. `Session::set_turn` puts the turn id on
  every origin, for Undo this turn (Prompt 39).

`vellum-engine-builtin` runs: prompt pack system prompt with the skills listed → the person's ask (and
pictures) → rounds of model request and tool calls → stop. Before it accepts a stop after a write, it asks
once for a self-check (`screenshot`, or `snapshot_layout` for a model without vision) unless a check already
followed the last write. Screenshot bytes are taken out of the result JSON (a marker stays) and sent as a
picture after the tool results, or not at all to a model that cannot see. Limits: rounds (40), failed calls (8),
tool result size (24,000 characters of JSON).

Prompt packs are files (`packages/prompt-packs/packs/builtin.md`: an `id`/`version` header and `system`,
`self-check`, `summarize` sections; the system section holds `{{skills}}` once). The shipped pack is compiled
in; a host can parse another. The text is written from scratch.

Context: tokens are estimated at four characters each (1,500 per picture); an unknown window counts as 128k.
Over 80% of the window left after the reply, results and pictures older than the last two rounds become
markers that name them; if that is not enough, the older rounds are summarized by a request with no tools
whose input is a plain transcript, and replaced by one labelled message holding the ask and the summary.
Too big even then is `ContextFull`. Nothing is dropped without a marker; `ToolFinished.truncated` and
`Compacted` report what was cut.

Stop cancels the turn's `Cancellation`; every wait on the provider races it, so a turn ends when the stream is
next polled, even if a provider ignores cancellation, and the dropped future closes the HTTP request. Calls
the model asked for but that did not run get a "stopped" result so history stays valid. The Guard side of
Invariant 12 (no transaction within 1 s) stays with Pause/Stop in Prompt 39.

The OpenAI-compatible adapter now sends a user message's tool results before its other content, which Chat
Completions requires when a picture follows a tool result.

## Reference note

goose `large_response_handler.rs` writes results over 200k characters to a temporary file and tells the model
the path; Vellum keeps a marked prefix instead, because the engine has no file system and agents can ask again
more narrowly. goose `context_mgmt` and opencode `compaction.ts` summarize with the model and keep recent
context, and opencode first clears old tool output; Vellum keeps both steps, and changes the summary input to a
plain transcript without tools so it works on every provider. No code was ported.

## Consequences

- A fake engine (`vellum_contracts::fakes::FakeEngine`) and contract suite exist in Rust. The TypeScript
  `engine-client` fake for UI work is not built; a later UI prompt adds it over the daemon protocol.
- Nothing wires the engine into the daemon or a `vellum prompt` command yet; sessions (34), permissions (35)
  and the composer decide that. The HTTP end-to-end tests are in the tagged `model-http` lane.
