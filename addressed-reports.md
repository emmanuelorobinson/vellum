# Addressed reports

Items moved out of `reports.md` "Needs a person" once resolved. Newest first. Decisions marked
"decided for the owner" were delegated by the owner on 2026-10-22.

## 2026-10-22 follow-up session

- **Prompt 55 document runtime (was: VS Code host opens read-only; WASM had no document ABI; no EditorLink wire).**
  Done per ADR-033: editors are Loro replicas of the daemon's engine (`vellum/editor` host RPC, actor-checked
  `DocStore::import_updates_as`, no autosave while an editor is attached, revision-based dirty state). The editor
  WASM module carries the engine (`WasmDocumentEngine`, `createLiveEditor`). Verified: native engine tests, browser
  e2e `engine.spec.ts` (draw, undo/redo, read-only), real VS Code smoke (agent edit → dirty → Save) and Electron
  folder test (same round trip). Also unblocks the "Prompt 57/58 opens read-only" items for editing.
- **Screenshots of frames holding note/context/prompt nodes failed (Prompt 29).** Decided: annotations are left
  out of screenshots and exports; the editor draws them as overlays.
- **Libraries and script outputs did not resolve in the daemon's scene (Prompt 29).** `Documents::services_for`
  builds per-document scene services (document fonts, libraries from disk, sandboxed script outputs), cached until
  the document changes; layout, find_space, execute and screenshots use it.
- **Pixel grid not drawn (Prompt 54).** Drawn on the overlay from 800% zoom; ⌘' / ⇧⌘' toggle it and snapping.
- **Codex joined agent messages with no space (Prompt 49).** Separate messages are separate paragraphs; fixture
  rewritten; prose keeps line breaks.
- **20 inverse-text contrast failures.** Exporter backdrop rule fixed for single-character initials on discs;
  tokens re-exported from the read-only design copy; `pnpm tokens:lint` passes.
- **17 DESIGN.md vs vellum.pen token gaps.** Accepted the design file's side (decided for the owner); recorded in
  DESIGN-GAPS.md; `pnpm tokens:check` passes.
- **Script tests failing on a clean checkout** (ADR-000 status, legacy DESIGN-GAPS grammar, reference index R3,
  postject prerelease). Fixed; full script suite 237 pass / 0 fail. postject recorded as version-baseline C27.
- **14 Rust advisories.** Fixed by upgrading rustls-webpki/webpki-root-certs and zip 9.0.5, and porting vtracer's
  converter over visioncortex; six unmaintained-only notices ignored with reasons (C28). `cargo deny check` passes.
- **pnpm advisories.** smol-toml 1.9.1 and Vitest 5.0.5 fix three. braces and sprintf-js (build tooling only, no
  patched release) remain.
- **Whole-repo Biome errors.** Fixed; byte-compared fixtures excluded from formatting.
- **Clean-room denylist empty.** tldraw/Penpot identifiers added.
- **Guide FINAL commands that did not parse** (Prompts 13–15 and others). Corrected in the guide.
