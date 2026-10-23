---
title: "Reference repositories"
description: "The open-source repositories the project reads for ideas and, where the licence allows, ports from, and the rule that applies to each one."
---

The project reads several open-source repositories to learn how hard problems have been solved. They are cloned into `refs/` at pinned commits, read
only, and never committed. `pnpm refs` clones them, which takes about 2 GB and needs the network.

## The rules

- **Read first.** Before you implement a hard problem that a reference solves, read the part of the reference that solves it, with `rg` on the named paths.
  Do not load whole directories.
- **Note what you took.** The pull request names the reference and says what the project keeps, what it changes, and why.
- **Port only what the licence allows.** A file is ported only from a project under a permissive licence, with a header that names the source and an entry in
  the third-party notices.
- **Never port from a copyleft or a commercial project.** Those are read for ideas, or their public documentation is read, and nothing is copied.

## The references

| Repository | Licence | Rule |
| --- | --- | --- |
| Penpot | MPL-2.0 | Read only. Never ported |
| Graphite | Apache-2.0 or MIT | Read, and ported with attribution where needed |
| Loro | MIT | Read, and ported with attribution where needed |
| Taffy | MIT | Read, and ported with attribution where needed |
| Parley | Apache-2.0 or MIT | Read, and ported with attribution where needed |
| Vello | Apache-2.0 or MIT | Read, and ported with attribution where needed |
| Kurbo | Apache-2.0 or MIT | Read, and ported with attribution where needed |
| Codex | Apache-2.0 | Read, and ported with attribution where needed |
| Goose | Apache-2.0 | Read, and ported with attribution where needed |
| OpenCode | MIT | Read, and ported with attribution where needed |
| rquickjs | MIT | Read, and ported with attribution where needed |
| rmcp | Apache-2.0, with MIT for legacy parts | Read, and ported with attribution where needed |
| VS Code samples | MIT | Read, and ported with attribution where needed |
| Excalidraw | MIT | Read, and ported with attribution where needed |
| tldraw | A commercial licence | Public documentation only. Never ported, never a dependency |

The version baseline records each reference's pinned commit and licence. A reference with a release newer than its pin keeps its pin, so every reader sees
the same code.
