# Runbook: reading the design through the Pencil MCP

The design is `docs/design/vellum.pen`, made in pen.dev. It is read-only specification. Agents read it
through pen.dev's MCP server (it shows up as `pencil` in the agent's MCP list) on a read-only copy, and CI
reads the committed snapshots and `inventory.json` instead. This page is the one place that says how to set
that up and what to do when it does not work. The session rules come from guide §2.1 and AGENTS.md.

## Set up (once per machine)

1. Install the pen.dev desktop app (or its IDE extension) and sign in.
2. In pen.dev open **Settings → MCP** and enable the integration for the coding agent you use (Claude Code,
   Codex, and so on). pen.dev writes that agent's MCP config entry.
3. Run `pnpm design:open` (the script is `scripts/design-open`). It copies `docs/design/vellum.pen`, and
   `docs/design/assets/` if there is one, to `.design-cache/vellum.readonly.pen` and opens that copy in pen.dev.
   The copy has no write bit and is replaced on every run, so an edited or stale copy never survives.
   `--no-open` copies without launching anything.
4. Restart the coding agent, then check that `pencil` is in its MCP list (`claude mcp list` for Claude Code).
5. For regenerating snapshots only: run `pen login`, or set `PEN_CLI_KEY`. Reading the design through the MCP
   needs neither.

## Rules for every session that reads the design

1. Call `get_app_state` first. The active document must be `.design-cache/vellum.readonly.pen`. If it is any
   other document, including the live source in `~/.pencil`, stop and ask.
2. Call `read_skill` once per session, and again with the `execute` page before the first `execute`. Do not
   write pen.dev's script syntax into code or docs. The one file that holds it is
   `scripts/lib/pen-queries.mjs`, and `design-snapshot` checks the current `execute` page against it first.
3. Read only. No `execute` that inserts, updates, moves, deletes or replaces, no `spawn_agents`, never save.
   Exporting images to `.design-cache/` is fine.
4. Ask for frames by screen ID (`C03`, `S07`), only the frames and components the prompt names. Never dump
   the whole document.
5. Compare a screenshot of the design frame with one of the implementation at the same size and theme.
6. Take colour, spacing, radius and type from the `.pen` variables and components. A hardcoded hex in UI code
   is a lint failure.
7. pen.dev's HTML export is a reference, never product code.

`pnpm checksum` and the test `design-spec-untouched` fail if `docs/design/vellum.pen` changes during a build
or test run.

## How the design file is named

The file does not follow the guide's §2.1 convention exactly. It has no pages (the .pen format has none) and
names frames `C03 · Agent workspace · building · 1512 · hc-light`: screen ID, title, a width only when it is
not 1280, and a theme that is one of `light`, `dark`, `hc-light`, `hc-dark`. Each difference is a line in
`docs/design/DESIGN-GAPS.md`. Find frames by screen ID, never by page.

## Regenerate the snapshots

Needed whenever `docs/design/vellum.pen` changes. A human changes the file on purpose, then runs
`pnpm checksum:update`.

From a terminal with the pen CLI signed in:

```text
pnpm design:open --no-open
pnpm design:snapshot
pnpm design:check
```

`design:snapshot` runs `pen interactive` headless on the read-only copy. It reads the `execute` page first and
stops if the functions it relies on are gone. It writes `docs/design/snapshots/inventory.json` and, for every
screen frame, a PNG at 1x and 2x named `<id>-<theme>-<width>.png` and `<id>-<theme>-<width>@2x.png`. It removes
snapshots of frames that no longer exist. Output is deterministic, so a second run changes nothing.

From an agent session with the Pencil MCP connected, when the CLI is not signed in:

1. `pnpm design:snapshot --print-query` prints the read-only query. Run it with `execute` on the copy and save
   its output to a file.
2. `pnpm design:snapshot --raw <file> --staging <dir> --print-export` prints two `Export` calls. Run both with
   `execute`.
3. `pnpm design:snapshot --raw <file> --staging <dir>` builds `inventory.json` and the snapshot files.

Snapshots are about 100 MB for the 252 screen frames at both scales.

## What `pnpm design:check` enforces

It needs no pen.dev, so CI runs it.

- `vellum.pen` hashes to `inventory.source_sha256`. If not, the snapshots are stale.
- Every screen ID DESIGN.md numbers, in light and dark at 1280 × 800, exists. C03, C05 and C10 also have
  `hc-light` and `hc-dark`. C03 and C05 also exist at 1512 × 982, and S13 at 960 × 640 and 760 × 640. Missing
  frames are listed by ID, theme and width, and a UI prompt that needs one must stop. The v1.1 screens (C31,
  C32) are warnings, because DESIGN.md says they can follow later.
- Frame names parse, sizes match the width in the name, no screen/theme/width appears twice, and every screen has
  both snapshot files.
- Each entry of DESIGN.md "Component library" matches a reusable component, directly or through
  `scripts/design-component-aliases.json`, or is recorded in DESIGN-GAPS.md with `check=component-family:<name>`.
- Variables fall in the groups chrome, canvas, agent, type (or font), space, radius and motion, are named
  `group/name`, and a theme axis holds light, dark, hc-light and hc-dark.

A recorded gap turns only the matching finding into a warning. A stale checksum, a missing screen and a
missing snapshot can never be excused.

## When the MCP is unavailable

The app is closed, you are not signed in, or you are in CI:

1. Use `docs/design/snapshots/` and `inventory.json`. UI prompts do not start without at least the snapshots.
2. Say so in the PR: "design read from snapshots, Pencil MCP unavailable".
3. If `pnpm design:check` fails because the snapshots are stale, regenerate them first (above) or ask a person to.
4. Do not fall back to reading `vellum.pen` as text. The file is encrypted and only the MCP or CLI reads it.
