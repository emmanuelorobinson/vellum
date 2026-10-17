# Design gaps (append-only)

Every place where `vellum.pen` and `DESIGN.md` (or the guide's description of the design file) disagree.
One line per mismatch. Never edit or delete a line: add a new one that supersedes it.

Line format:

```text
- <date> · <area> · DESIGN.md or guide says: <text> · vellum.pen has: <text> [· check=<key>]
```

`check=<key>` ties a line to a finding of `scripts/design-check`. A finding with a recorded key is shown as a
warning instead of an error, so a known difference does not hide a new one. A missing screen, a stale
checksum and a missing snapshot can never be excused this way. Code follows `vellum.pen` where it is
specific, because the guide ranks the design file above itself (§2); the line says which side the code takes.

## Recorded

- 2026-10-11 · frame names · guide §2.1 says: `"<ScreenID> <Title> / <Theme> / <Width>"`, e.g. `C03 Agent workspace / Dark / 1280` · vellum.pen has: `"<ScreenID> · <Title> · [<Width> · ]<theme>"`, e.g. `C03 · Agent workspace · building · 1512 · hc-light`, width in the name only when it is not 1280 · code reads the file's form (`scripts/lib/design-frames.mjs`)
- 2026-10-11 · themes · guide says: Light, Dark, High contrast · vellum.pen has: four appearances, `light`, `dark`, `hc-light`, `hc-dark`; DESIGN.md asks for high contrast on C03, C05 and C10, and the file has both `hc-light` and `hc-dark` for them · code uses the four names
- 2026-10-11 · pages · guide §2.1 says: pages Tokens, Components, Editor Agent, Editor Canvas, Editor Code and Files, Desktop and Web, Edge States · vellum.pen has: no pages (pen.dev's .pen format has none); one flat document of 313 top-level frames · `inventory.json` records `page: null`; frames are found by screen ID, never by page
- 2026-10-11 · non-screen frames · guide says: nothing · vellum.pen has: sheets and fixtures beside the screens (`Library · …`, `Components · …`, `Flow · …`, `Foundations · …`, `CLI · …`, `Vellum · …`, `State-… · …`, `Fixture-… · …`) and secondary states with an id suffix (`C10-revert`, `W02-uploading`) · code accepts exactly these kinds; any other prefix is an error
- 2026-10-11 · variable theme axis · guide says: one axis named `Mode` with Light, Dark, High contrast · vellum.pen has: one axis named `appearance` with `light`, `dark`, `hc-light`, `hc-dark` · design-check accepts any axis that holds all four values; Prompt 43 maps `appearance` to the token modes
- 2026-10-11 · variable groups · guide says: one collection per group (chrome, canvas, agent hues, type, space, radius, motion) · vellum.pen has: names of the form `group/name` in those groups, with the type group called `font` (`font/ui`, `font/mono`, `font/arabic`) · design-check accepts `type` or `font`
- 2026-10-11 · legacy variable names · guide says: names match `group/name` · vellum.pen has: five names without a group, `vellum-ink`, `vellum-muted`, `vellum-line`, `vellum-paper`, `vellum-teal` (only the `Vellum · Landing page` frame uses them) · check=variable-name:vellum-ink · check=variable-name:vellum-muted · check=variable-name:vellum-line · check=variable-name:vellum-paper · check=variable-name:vellum-teal
- 2026-10-11 · component names · guide says: components named exactly as DESIGN.md's Component library, e.g. `Agent Panel / Step row`, `Actor Marker / Agent / 24` · vellum.pen has: `Component / <Family> / <State>`, e.g. `Component / Actor / teal / 24`, `Component / Ledger / Expanded`, plus 63 other reusable nodes named by what they are (`Apply button`, `Gap field`) · design-check matches each Component library entry through `scripts/design-component-aliases.json`; Prompt 43 reads the file's names
- 2026-10-11 · component library entry without a reusable component · DESIGN.md says: **Script node** (rendering, error, empty) · vellum.pen has: screens C28, C28-error and C28-empty, no reusable component · check=component-family:Script node
- 2026-10-11 · component library entry without a reusable component · DESIGN.md says: **Script section** with generated input controls and Convert to layers · vellum.pen has: the section drawn inside C28, no reusable component · check=component-family:Script section
- 2026-10-11 · component library entry without a reusable component · DESIGN.md says: **blend mode menu** · vellum.pen has: a Blend value row in the fill editor frames, and no menu that lists the blend modes in any frame or component · check=component-family:blend mode menu
- 2026-10-11 · component library entry without a reusable component · DESIGN.md says: **Canvas nodes for collaboration** (sticky note, context block, prompt node with Run, embed) · vellum.pen has: the nodes drawn inside C35 and C31-embed, no reusable components · check=component-family:Canvas nodes for collaboration
- 2026-10-11 · component library entry without a reusable component · DESIGN.md says: **Notes for agents** field · vellum.pen has: the field drawn inside C12, no reusable component · check=component-family:Notes for agents
- 2026-10-11 · component library entry without a reusable component · DESIGN.md says: **Guidelines editor** · vellum.pen has: the editor drawn in C12-guidelines, no reusable component · check=component-family:Guidelines editor
- 2026-10-17 · token value · DESIGN.md says: Tertiary text #7B8089 (light), #7E838B (dark) · vellum.pen has: `chrome/tertiary` #686D75 (light), #A3A7AE (dark, the same as `chrome/secondary`) · code uses vellum.pen; awaiting a person (Prompt 43): accept by adding a line with the key token-value:chrome/tertiary, or change one side
- 2026-10-17 · type tokens · DESIGN.md says: ten type roles (Window title, Dialog title, Agent prose, Agent meta, Panel section, Panel body, Panel caption, Canvas label, Mono value, Mono block) with size, line height and weight · vellum.pen has: no type variables, only `font/ui`, `font/mono`, `font/arabic`; frames set sizes directly · no type-scale tokens are generated; awaiting a person (Prompt 43): add the variables to vellum.pen, or accept by adding a line with the key token-missing:type/<role> for each role
- 2026-10-17 · size tokens · DESIGN.md says: nothing about toolbar, status bar or ledger row heights · vellum.pen has: `chrome/toolbar-height` 44, `chrome/status-height` 24, `chrome/ledger-height` 32 · code uses vellum.pen; awaiting a person (Prompt 43): accept by adding a line with the key token-extra:<name> for each, or add them to DESIGN.md
- 2026-10-17 · elevation tokens · DESIGN.md says: "floating panels share one soft, low shadow; menus and dialogs a slightly stronger one" (no values, two levels) · vellum.pen has: one level, `chrome/elevation-color` (#17181B24 light, transparent otherwise) and `chrome/elevation-blur` 16 · code uses vellum.pen; awaiting a person (Prompt 43): accept by adding a line with the keys token-extra:chrome/elevation-color and token-extra:chrome/elevation-blur, or add a second level
- 2026-10-17 · motion token · DESIGN.md says: "toasts slide 8px" (no duration) · vellum.pen has: `motion/toast` 160 · code uses vellum.pen; awaiting a person (Prompt 43): accept by adding a line with the key token-extra:motion/toast, or add the duration to DESIGN.md
