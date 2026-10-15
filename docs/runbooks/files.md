# Files on disk

How a host keeps a document, its assets, its fonts and its libraries (guide Prompt 24, ADR-018). The format itself is
in `docs/format/files.md`, generated; this page is for people wiring a host.

## The pieces

| Need | Use | Where |
|---|---|---|
| A real disk | `StdFs::new(root)` | `vellum-assets-fs` |
| A document at a path, one file | `SingleFile::new(&codec)` | `vellum_core::files` |
| A document at a path, a folder | `DirCodec::new(&codec)` | `vellum-codec-json-dir` |
| The assets beside a document | `FsAssets::new(fs, assets_dir_for(path))` | `vellum-assets-fs` |
| Remove unused assets on an explicit save | `collect_on_save(SaveKind::Explicit, ...)` | `vellum_core::files` |
| Import a font | `ParleyFonts::import_into_document` then `Op::SetShell` | `vellum-text-parley` |
| Serve a document's fonts | `ParleyFonts::load_document_fonts` | `vellum-text-parley` |
| Turn a document into a library | `turn_into_library` then `Op::SetShell` | `vellum_core::files` |
| Read libraries, keep last-known copies | `FileLibrarySource`, `LibraryCache` | `vellum_core::files` |
| Carry bytes on a library paste | `copy_dependencies` | `vellum_core::files` |
| Suggest the folder format | `suggest_multi_file(count_nodes(&doc), json_len)` | `vellum_core::files` |

The composition root passes the same single-file codec to both `SingleFile` and `DirCodec`; the two formats then agree
on every byte inside a file.

## Rules a host has to keep

- One writer per path. `atomic_write` shares one temporary file per target.
- Name documents the same way every time, relative to the root of the `FileSystem` handed to the library code. The
  library cache is shared by every project on the machine and keyed by that path. Root `LibraryCache` at
  `~/.vellum/cache/` and give it a folder such as `libraries`.
- Call `collect_on_save` with `SaveKind::Explicit` only when the person saved, and pass the assets of anything the
  undo history can still bring back as `also_keep`.
- One `ParleyFonts` per open document.
- Show `Loaded::interrupted` and `Loaded::orphans` to the person. Do not hide them.

## Run the tests

```sh
cargo test -p vellum-contracts --test files --test document_files
cargo test -p vellum-assets-fs -p vellum-codec-json-dir -p vellum-text-parley
cargo test -p vellum-codec-json --test library_files
cargo test -p vellum-core --test files_assets
cargo test -p vellum --test files
pnpm format-spec:check
```

## Re-measure the large-file threshold

```sh
cargo test -p vellum --release --test files -- --ignored --nocapture measure
```

It prints one CSV row per size (2,500 to 40,000 nodes): JSON size, whether the suggestion fires, encode and decode
time, and the saves on a real disk. Copy the table into ADR-018 when the numbers change.
