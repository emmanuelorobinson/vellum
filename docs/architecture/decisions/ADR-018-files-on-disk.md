# ADR-018: files on disk — assets, custom fonts, libraries, the directory format

Status: implemented for Prompt 24. Only macOS arm64 was executed; Linux and Windows disks are not verified.
Date: 2026-10-15. Supplements ADR-004 (the directory-codec threshold) and ADR-011 (library scope, "persistence is
Prompt 24"). The document format version is unchanged (1); `docs/format/files.md` is new and generated.

## Decisions

1. **A `FileSystem` port in `vellum-core`, and `atomic_write` on top of it.** Core still touches no disk. The port
   is whole files, folders, `rename`, and the two syncs a durable write needs; paths are relative to a root the
   implementation owns, use `/`, and refuse `..`, a leading `/`, an empty part, `\` and NUL. `atomic_write` writes
   `.<name>.vellum-tmp` beside the target, syncs it, renames it over the target and syncs the folder. `StdFs`
   (`vellum-assets-fs`) is the real implementation. `MemFs` (`vellum-contracts`) is the fake, and it can be crashed:
   after N changing calls everything fails, and `after_crash()` returns what a restart finds, where a file written
   but not synced comes back with half its bytes. Every crash test pulls the plug at every step. Deleting the sync
   from `atomic_write` makes them fail (checked by hand once).
   *Not modelled:* a rename lost because its folder was never synced. Renames are all or nothing in the fake, so a
   missing `sync_dir` is not caught.

2. **Assets are one file per SHA-256, named by it, with no extension.** `<name>.assets/<64 hex>`. The media type is
   not part of identity (the contract already says the hash depends on the bytes alone), so it is not stored.
   `get` re-hashes and reports a mismatch as damaged; `put` rewrites a file that is present but wrong instead of
   trusting it. `AssetCollect` (`list`, `remove`) is a separate port from `AssetStore` because an S3 store may offer
   neither. `collect_on_save` removes unreferenced assets only for `SaveKind::Explicit`, and takes an `also_keep` set
   for assets the undo history can still bring back. It reports used ids missing from the store, never invents them.
   "Referenced" is found by reading the document as JSON and taking every `asset` field, every key of `fonts` and
   every `src` of the form `<x>.assets/<hash>`, so a place that gains an image later is covered. Over-keeping is the
   only error it can make.
   *Difference from ARCHITECTURE §4.3:* it says shader files live in `<name>.assets/` by hash. The fixtures reference
   shaders as plain relative paths (`shaders/glow.sksl`) and `ShaderEffect.src` is a path, not an `AssetId`. A `src`
   into an assets folder is an asset; any other path is a plain file, like a script.
   *Dependency:* `sha2` 0.11.1, banned everywhere but `vellum-assets-fs` (`deny.toml`) and kept out of core
   (`scripts/rust-deps-allowlist.txt`). Baseline C20.

3. **Custom fonts: stored as imported, unpacked on load, served first.** TTF, OTF, WOFF and WOFF2 files go into the
   assets exactly as given (a WOFF2 stays a WOFF2) and are listed in `Document.fonts` as
   `AssetId → FontFace`. `ParleyFonts::import_into_document` checks the file is a font *before* anything is stored and
   returns the `Shell` to apply with `Op::SetShell`, so the write path is still an op. `load_document_fonts` reads each
   file from the `AssetStore`, unpacks it with `wuff`, and registers it at `FontTier::Document` under the family,
   weight and style *the document* gives it (the document is what text runs ask for), so a family in the document
   beats the bundled one. A font that cannot load is a `FontProblem` naming the family and the reason (missing,
   damaged, not a font, could not be unpacked); the others still load. A second file for the same family, weight and
   style is refused (`ImportError::Duplicate`). A header that claims more than 128 MiB unpacked is refused before
   anything is allocated. One `ParleyFonts` holds one document's fonts.
   *Dependency:* `wuff` 0.2.9 (pure Rust, MIT), banned everywhere but `vellum-text-parley`. Baseline C20.
   *Tested with:* real Inter Latin WOFF and WOFF2 from `@fontsource/inter` 5.3.0 (OFL) under
   `crates/adapters/text-parley/tests/fonts/`, every truncation of both, and a bundled TTF. No OTF with CFF outlines
   was available, so OTF is covered by container sniffing only.

4. **Libraries.** `files::turn_into_library` returns a `Shell` with `meta.kind = library` (refusing a library); the
   editor applies it with `Op::SetShell`. The one-way rule is the helper's, not `SetShell`'s: `SetShell` is also how an
   external change reaches a document (`git checkout` of an older file is a diff, applied as ops), so refusing
   library → document there would make that checkout unappliable. Undo restores the document, and no command turns it
   back. `library_path_for` names the file (`app.vellum` → `app.lib.vellum`). `FileLibrarySource` reads libraries
   through a `FileSystem` and a `Codec`: a file that is not there is `None`, one that cannot be read says why in the
   words of S05. `LibraryCache` keeps a last-known copy of every library the resolver knows
   (`<name>-<fnv1a of the path>.lib.vellum` plus a `.source` sidecar holding the path, so a collision or stray file is
   skipped with a reason, never shown as another library). `persist` skips copies already on disk; `warm` seeds
   `Libraries` before the first resolve pass, after which the resolver's own rules apply (the library is flagged
   missing and the copy renders). `copy_dependencies` carries asset and font bytes named by a `PastePlan` from one
   `AssetStore` to another and reports what is missing; scripts and shaders beside the document are reported as files
   for the host.
   *Needs from the host:* document paths must be the same names for the same file every time, relative to the root of
   the `FileSystem` it passes (an absolute path is read from that root), because the cache is shared by every project.

5. **The directory format is a `DocumentFiles`, not a `Codec`.** ARCHITECTURE §4.3 calls it a second `Codec`, but
   bytes in and bytes out cannot say which page a byte belongs to. `DocumentFiles` reads and writes a document at a
   path of a `FileSystem` and returns what it found (`orphans`, `interrupted`) or did (`written`, `unchanged`,
   `removed`). `SingleFile` is the same port over a `Codec`, so the two formats are used the same way, and switching is
   read with one and write with the other. Layout:
   `app.vellum/index.json` (a normal document whose pages are empty stubs, in order),
   `app.vellum/pages/<page-id>.json` (a normal document holding one page) and `.saving`. Every file is written by the
   injected single-file codec, so quantization, migrations and error positions are the codec's, and an error in a page
   names its file (S05 keeps its own line and column). A page file's meta is fixed (the page's name, the current
   version, `createdWith` `vellum-json-dir`), so renaming the document rewrites only the index and editing a page only
   that page. Page ids that differ only in case, and Windows' reserved device names, are refused before anything is
   written. Adapters do not import each other, so `codec-json-dir` takes the codec as `&dyn Codec`; the tests with the
   real codec live in `crates/vellum/tests/files.rs` (every fixture, both directions, byte for byte).
   *Saving.* A save that changes one file is one atomic write. A save that changes several writes `.saving`
   (durably), the changed pages, the index, removes page files the index no longer names, then the marker. Every file
   is whole at every crash point (simulated at each step), but pages can be from different moments, and the leftover
   `.saving` is how a read says `interrupted`. The next save, even one with nothing to change, finishes the job.
   Files in `pages/` the index does not name are `orphans`: never read, and a write removes the ones named `*.json`
   and leaves anything else.

6. **S06 threshold: measured, rule unchanged.** `files::LARGE_FILE_NODES` = 10,000 and `LARGE_FILE_BYTES` =
   4,000,000, whichever first (ADR-004), in `suggest_multi_file`. Re-measured with the real codec, Apple M5 Pro,
   release build, APFS, ADR-004's scene (one frame of 99 solid-filled rectangles per frame, ten pages), one run per row, so
   read them as magnitudes:

   | Nodes | JSON | Encode | Decode | Single-file save (disk) | Directory, one page edited (disk) | Directory, first save (disk) |
   |---:|---:|---:|---:|---:|---:|---:|
   | 2,500 | 1.18 MB | 3.9 ms | 9.0 ms | 11.1 ms | 11.0 ms | 91.7 ms |
   | 5,000 | 2.35 MB | 5.3 ms | 12.1 ms | 14.1 ms | 16.8 ms | 124.0 ms |
   | 10,000 | 4.71 MB | 11.1 ms | 24.4 ms | 18.2 ms | 21.2 ms | 114.7 ms |
   | 20,000 | 9.42 MB | 19.0 ms | 46.3 ms | 29.1 ms | 32.7 ms | 101.9 ms |
   | 40,000 | 18.86 MB | 39.3 ms | 94.7 ms | 52.0 ms | 57.1 ms | 129.9 ms |

   ADR-004 expected about 4 MB at 10,000 nodes; the real file is 4.71 MB (1.18 times, inside its "more than twice"
   revisit condition), so the numbers stand. For this scene the byte limit trips first, at about 8,500 nodes.
   **ADR-004's reason is corrected:** "the directory codec rewrites only the pages that changed" does not make saving
   faster. Every save still encodes every page (the codec is the cost: 11 ms at 10,000 nodes) to find what changed,
   and each file written is an fsync, so a one-page directory save is about as slow as a single-file save (21 ms
   against 18 ms at 10,000 nodes) and the first save is five to ten times slower. What a folder buys is Git: a diff, a
   merge or a conflict touches one page's file instead of one 4.7 MB file. The suggestion stays quiet and never blocks.
   `cargo test -p vellum --release --test files -- --ignored --nocapture measure` reproduces the table.

7. **The spec is generated, with real files.** `docs/format/files.md` (new, via `pnpm format-spec:gen`) describes the
   files on disk, assets, fonts, libraries, the directory format and the large-file rule; shows the files both codecs
   really write for one small document (`crates/vellum/examples/format-files.rs`); and lists the migration steps (none
   yet: format 1 is the first). The README links to it.

## Costs and risks accepted

- A crash during a multi-file directory save can leave pages from different moments. Detected (`interrupted`), not
  prevented, and nothing here repairs it: the daemon's recovery copy (Prompt 28) is the answer. Per-page checksums
  in a custom index would let a reader say which pages are stale, at the cost of an index that is not a `.vellum`
  document.
- Saving a directory encodes every page. Skipping unchanged pages needs the change set of the ops since the last
  save, which the save path does not have yet.
- The library cache keeps the document only. A library's images and fonts are not cached, so a cached copy of a
  deleted library renders text and shapes but its images are missing.
- `SetShell` can flip a library back to a document, so an agent transaction could. The Guard (Prompt 13) does not
  refuse it.
- A WOFF2 decoder bug in `wuff` would be ours; its README claims byte-identical output to Google's woff2 for every
  font in google/fonts.
- Symbolic links inside a document folder are followed like any other entry.

## Not verified

- Linux and Windows disks: `sync_dir` is a no-op on Windows, `rename` over an existing file relies on the platform.
  Only macOS arm64 (APFS) ran.
- A real power cut. The crash tests are a model, pessimistic about unsynced data and silent about unsynced renames.
- OTF with CFF outlines, font collections with several families, and a 100 MB font.
- Large-file measurements on an M1 or a Windows laptop, and on a slow disk.

## Revisit when

- Directory saves need to be faster than single-file saves for large documents: add a dirty set to skip encoding
  unchanged pages, and measure again.
- A real document is more than twice the sizes in the table at the same node count (ADR-004's condition, restated).
- A mixed directory after a crash shows up in practice: add per-page checksums, or write the index first and make
  page files content-addressed.
- Pasting from a deleted library has to show its images: cache the library's assets too.

## Reference reading

No row of guide §3.2 is assigned to Prompt 24. `refs/` was not searched; sibling projects were not either, because the
work is file handling over this repository's own ports.
