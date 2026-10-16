# ADR-006: Session storage, usage and cost, turn state machine

Status: proposed; implemented in Prompt 34. Closes delta D6 (ADR-000). ARCHITECTURE §3.4, §5 and §7.6 synchronized
on the owner's instruction (2026-10-16).

## Reference note (guide §3.2 R16)

Codex keeps each thread as an append-only JSONL rollout and puts queryable metadata in SQLite; its
`ThreadStore` separates the raw history append from the one metadata-write API. Goose keeps sessions,
messages and a usage ledger in one SQLite database (WAL, foreign keys, `ON DELETE CASCADE` on the ledger,
a `schema_version` table) and names sessions from the first messages. Vellum keeps Codex's split (history is
append-only; renaming is the only metadata write) and Goose's single SQLite file with cascades, but stores the
event log in SQLite too (no JSONL) so one transaction covers an append and its picture references, and moves
pictures out of the database into a content-addressed cache. Nothing was ported.

## Decision

**Port.** `SessionStore` in `vellum-core::ports`: `create`, `sessions(&DocKey)` (newest activity first),
`session`, `rename`, `append(&SessionId, NewEvent) -> EventSeq`, `events(&SessionId, since)`, `put_image`,
`image`, `delete`. Every call returns `Result<_, StoreError>` (ARCHITECTURE's sketch has infallible reads, which
SQLite cannot promise). `create` and the picture calls are additions; ARCHITECTURE has no way to start a session
or reach the cache.

**Keys.** `DocKey` is an absolute repository root plus a `/`-separated path inside it (no `.`, `..` or empty
segments). The daemon finds the root as the nearest enclosing folder holding `.git` (a folder or a worktree's
file); a document outside any repository is keyed by its own folder. Renaming a branch changes neither.

**Events.** An event is a time, an `Origin` (actor, session, turn) and a body: `Prompt` (text, context items,
pictures), `Messages` (what a turn added, for the next turn's history), `Plan`, `Step` (tool, arguments and
result previews cut at 2,000 characters with a marker, thumbnail, start, duration, outcome
done/failed/rejected, and the Agent panel's step detail), `Usage`, `Turn` (from, input, to). The store
assigns places from 1 with no gaps. Bodies are stored as the core's JSON (`vellum_core::sessions::body_to_json`),
which refuses unknown fields and types on read, so every store reads the same log the same way. Stores check
that the origin's session is the session appended to, that a `Turn` event carries its turn, and that every
picture named is already cached.

**External sessions** (MCP clients) keep `Step` events only; `SessionKind::accepts` is the rule and every store
enforces it. The daemon records a connection's steps after each `tools/call`, one session per document: the
first document uses the connection's own session id (so History's origins match the log), later ones get fresh
ids. Steps before the connection names any document are not recorded. Steps now note the first document they
read, not only the one they wrote.

**Pictures** live in `~/.vellum/cache/<sha-256 hex>`, written to a temporary name and renamed. Messages store
pictures as references; `vellum_core::sessions::history` reads them back for `TurnRequest::history`.
Thumbnails are drawn by the `screenshot` path at the step's scale; a frame that cannot be drawn (today, one
holding a note, context or prompt node) gives a step without a picture.

**SQLite** (`vellum-sessions-sqlite`, rusqlite 0.40.2 with bundled SQLite) at `~/.vellum/sessions.db`, mode
0600 in a 0700 folder: `sessions` (index), `events` (`WITHOUT ROWID`, primary key session+seq, a trigger refuses
every `UPDATE`), `event_images` (which event names which picture). Foreign keys cascade, so deleting a session
removes its events and references in one transaction; afterwards pictures no remaining event names are deleted
from the cache (a crash in between leaves an unnamed file, never a dangling reference). Schema version is
`PRAGMA user_version` = 1; a newer database is refused, not opened. WAL journal. A daemon whose store cannot
open still serves tools and records nothing.

**Usage and cost.** `ModelUsage` now has one meaning for every provider: `input_tokens` includes cache reads
and writes, `output_tokens` includes reasoning, and `cache_write_tokens` is new. The Anthropic adapter adds its
separately reported cache reads/writes into input; the Gemini adapter adds thoughts into output. A `Usage` event
holds `Tokens { input (uncached), output, cache_read, cache_write }` and an estimate in micro-USD computed when
recorded. `pricing.json` (`crates/vellum/data/pricing.json`, compiled in) is versioned data: prices in micro-USD
per million tokens per kind, optional aliases, unknown fields refused. The estimate rounds half up once over the
sum; a model or a used kind without a price gives no estimate, and session totals count those requests
(`unpriced`) instead of guessing. Only Anthropic rates are listed so far.

**Turns.** `vellum_core::turns` is guide §10.1 as a table: eight states, ten inputs, Stop from any
non-terminal state for a person only (`TurnRejection::NotAPerson` otherwise), terminal states reject
everything. `stuck(started, now)` is true after more than 120 s and changes nothing. The session dot is
total; `stopped` shows as done ("Stopped by you"), since stopping keeps what was built. `record_transition`
reads the turn's state from the log, applies the input, and appends exactly one `Turn` event with the actor;
a rejected input appends nothing. An engine's `Ended(Done | Failed)` becomes `done`/`error`; `Ended(Stopped)`
adds nothing because the person's Stop already moved the turn.

## Consequences

- `vellum_core::base64` replaces two private encoders (screenshot, SVG export) and decodes message pictures.
- `vellum-contracts` has `MemorySessions` (ARCHITECTURE's `memory`) and `suites::sessions`; the suite rejects
  six broken stores.
- Not decided here: who creates an engine session and its title (Prompt 49/50 hosts; `Session::log_as`
  registers it), retention or size limits for the cache, search over sessions (R16 says searchable; nothing
  needs it yet), and transaction ids on steps (the guide's "Two history layers" link; core has no
  transaction id type yet).
