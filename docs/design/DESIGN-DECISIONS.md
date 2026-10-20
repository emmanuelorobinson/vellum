# Design decisions (edited by a human only)

Choices the brief leaves open. If vellum.pen already shows one option, record it here so code
and design agree; if it shows both, pick one.

| Decision | Options | Choice | Date | By |
|---|---|---|---|---|
| Agent turn layout | A Prose and steps interleaved in order · B Prose first, steps collapsed below | A Prose and steps interleaved in order | 2026-10-18 | Owner (chose A when asked, Prompt 49) |
| Plan card placement | A Inline in the turn · B Sheet over the panel | A Inline in the turn | 2026-10-18 | Owner (approved defaults, Prompt 51) |
| Default plan threshold | 2 frames · 3 frames · 20 changes | 3 frames | 2026-10-18 | Owner (approved defaults, Prompt 51) |
| Lease tab position | A Above the frame's top-left · B Inside the frame's top edge | A Above the frame's top-left | 2026-10-18 | Owner (approved defaults, Prompt 51) |
| Placeholder style | A Dashed outline + shimmer · B Tinted fill + label | A Dashed outline + shimmer | 2026-10-18 | Owner (approved defaults, Prompt 51) |
| Alternatives layout on canvas | A Row with A/B/C labels · B Grid for 4+ | A Row with A/B/C labels | 2026-10-18 | Owner (approved defaults, Prompt 51) |
| Inline prompt anchor | A Beside the selection · B Below the selection toolbar | A Beside the selection | 2026-10-18 | Owner (chose A when asked, Prompt 50) |
| Slot marker | A Dotted inner outline · B Hatched fill | A Dotted inner outline | 2026-10-18 | Owner (chose A when asked, Prompt 52) |
| Selection toolbar contents | A Ask only · B Ask + quick actions | A Ask only | 2026-10-18 | Owner (chose A when asked, Prompt 50) |
| UI golden tolerance vs design snapshots | 0.5 % · 1 % · per-screen | 1 % | 2026-10-17 | Owner (delegated to agent, Prompt 43) |

## Approved v1.1 choices

The owner approved the supplied Prompt 64 handoff defaults in chat on 2026-10-20.

| Decision | Options | Choice | Date | By |
|---|---|---|---|---|
| Comments entry and placement | Fifth tab · Presence bar + C, right overlay | Presence bar + C, right overlay | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
| Comment authorship and resolution | People or agents; author only · People only; author or Editor resolves/reopens; no deletion controls | People only; author or Editor resolves/reopens; no deletion controls | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
| Live collaboration and snapshot scope | Combined sharing · Independent; live off by default; Share entry | Independent; live off by default; Share entry | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
| Multiplayer actor marks and hues | Shared marks and agent hue · People circles; agents 30% rounded squares with distinct assigned hues | People circles; agents 30% rounded squares with distinct assigned hues | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
| Stale presence and following | Live-looking stale cursors · Hide stale remote cursors; follow exits with Esc or Return to your view | Hide stale remote cursors; follow exits with Esc or Return to your view | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
| Role loss and room reset recovery | Discard or auto-rebase · Preserve drafts and unsent edits; explicit review/rebase | Preserve drafts and unsent edits; explicit review/rebase | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
| Sharing management | Any Editor · Owner manages live roles and snapshots; deleting a snapshot and removing live access are separate | Owner manages live roles and snapshots; deleting a snapshot and removing live access are separate | 2026-10-20 | Owner (approved Prompt 64 handoff defaults in chat) |
