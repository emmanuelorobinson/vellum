# ADR-020: document targets, the file watcher and recovery copies

Status: implemented for Prompt 28 as a library in the `vellum` composition root. Tools reach it from
Prompt 29. The host relay that connects real editors comes with the hosts.

## Decision

**Ports.** `vellum-core` gains two ports. `DocumentTarget` has `mode`, `snapshot`, `transact`,
`render`, `save` and `is_dirty`, with a typed `TargetError` (`Write`, `ReadOnly`, `Gone`, `File`,
`Render`). `FileWatcher` has `watch`, `unwatch` and a non-blocking `changed`. `changed` returns
document names only: the watcher never reads content. Each port has a fake (`InMemoryTarget`,
`ManualWatcher`), a contract suite, and a broken implementation the suite rejects.

**Targets** (`crates/vellum/src/daemon/`). `HeadlessTarget` loads a file through its format (one
file, or a folder in the directory format). It applies transactions through the Guard and saves
after every one, because no editor is there to press Save and the file is the truth. When a save
fails, the change stays in the engine, the target reports dirty, and the next save retries
(§10.3 `save_failed`). It renders with the same `doc_api::Services` that `doc.screenshot` uses.

`LiveTarget` forwards to an `EditorLink`, the daemon's end of one editor connection. The link
carries Rust `Transaction`s in process. Its wire encoding is left to the host relay, because ops
are not serialisable yet.

**One writer per file.** `Documents` holds one target per path. The first editor becomes the writer.
A headless target hands over only once everything is saved. Later editors attach: they are shown
the writer's document, unsaved changes included, and follow every applied transaction. When the
writer closes, the first attached editor is promoted. When nobody is left, the daemon lets go, and
the next tool call opens the file headless again. A writer that answers `Gone` is dropped, and the
daemon falls back to the file. Opening a file checks that the writer is still alive before
attaching to it. Without the daemon, an editor cannot know who the writer is, so it opens read-only
(`Role::OFFLINE`).

**External changes.** The watcher's report is only a hint. The daemon reads the file again and
hashes its canonical encoding (SHA-256 via `vellum_assets_fs::content_hash`). If the hash equals the
last known file, the change is a self-write or a no-op touch, and the editor is not asked anything.
Otherwise the daemon runs the three-way `merge(last file, engine document, new file)`, diffs the
engine document against the result, and applies those ops as one transaction by
`Actor::File { reason: ExternalChange }`. This keeps unsaved edits. A property changed on both sides
becomes a merge conflict (conflict node where node-level). A headless target does not write back a
change that came from the file, unless the merge kept something the file lacks.

Other outcomes are typed, and none of them applies anything:

- `Unreadable`: text-merge conflict markers, or a half-written file.
- `Interrupted`: a folder save with `.saving` present.
- `Missing`: the file was deleted or checked out away.
- `Refused`: the merge or the target refused the change.

**The notify adapter** (`vellum-watch-notify`, notify `=8.2.0`, its own debounce). Each document is
watched through its parent folder, non-recursively, so atomic-save renames and checkouts that
replace the file are followed. A folder document also gets a recursive watch, which is added when
the folder appears. Read-only access events are ignored. An OS error marks every document as
changed, and the hash check sorts out which ones really did. Paths are compared canonically
(`/var` vs `/private/var` on macOS).

**Recovery copies.** For a live file with unsaved changes, `tick` writes a copy to
`~/.vellum/recovery/` at most every 5 s (`RECOVERY_INTERVAL_MS`). The copy is two atomic files named
by a hash of the path: `<key>.vellum.recovery` (canonical document, matching the `.gitignore`
pattern) written first, then `<key>.json` (`path`, `savedAt`).

- **Removed:** on any save, on Discard backup, and on a deliberate close of the last writer. A lost
  writer keeps its copy.
- **Listed:** newest first, with damaged copies listed separately, never dropped. `open_editor`
  returns the copy for A06.
- **Restored:** `restore` applies the copy as one transaction by `Actor::File { reason: Recovery }`.
  The copy stays until the file is saved.

`recovery_dir` makes the folder 0700 and refuses symlinks and folders other users can read.

## Consequences

- `sha2` stays in `vellum-assets-fs`. The composition root gets the hash as a plain function.
- `crates/vellum` is now a library plus a binary. It depends on `vellum-core` and
  `vellum-codec-json-dir`, and does not build Skia. Headless rendering is injected, so the IPC lane
  still needs no renderer build.
- Merge as the external-change path means a checkout with unsaved edits is a merge, not an
  overwrite. ARCHITECTURE §4.5 says "diffs old vs new". With no unsaved edits the result is the same.
- Not decided here:
  - the editor wire protocol;
  - wiring `Documents` into the running daemon process with a watcher loop (Prompt 29 adds the first
    caller);
  - a production `IdGen`;
  - detecting `git merge` to attribute `FileReason::Merge`. The merge driver CLI is a later prompt.

## Reference note

notify-rs/notify was read through its published docs only (refs/ has no clone). From it, Vellum
keeps the recommended watcher and per-folder watches. It replaces the debouncer crate with a small
per-document debounce, and watches the parent folder so renames are followed. Nothing was ported.
