# Runbook: design tokens

`@vellum/ui-tokens` is generated from the design file's variables (guide Prompt 43). Nothing in it is typed by
hand except the two maps in `packages/ui-tokens/source/` that say how DESIGN.md's words line up with the
variables.

```text
docs/design/vellum.pen ──(read-only query)──▶ packages/ui-tokens/source/tokens.json ──▶ src/generated/tokens.css
                       └─(Export, Tokens sheet)─▶ docs/design/snapshots/foundations/Tokens.png      └▶ src/generated/tokens.ts
```

| File | What it is |
|---|---|
| `source/tokens.json` | every variable (name, group, type, value per appearance), the design file's sha256, and every text/icon colour pair the design uses. Generated; never edit |
| `source/design-md.json` | DESIGN.md "Visual system" role → variable map, plus word-for-word quotes for values DESIGN.md gives in prose |
| `source/contrast-pairs.json` | pairs the contrast lint checks besides the ones the design uses |
| `src/generated/tokens.css` | CSS custom properties: scalars on `:root`; chrome colours under `[data-appearance=…]`; canvas and agent colours under `[data-canvas-appearance=…]` |
| `src/generated/tokens.ts` | `colors`, `scalars`, `MODES`, `SOURCE_SHA256`; `src/index.ts` wraps them (`cssVar`, `colorValue`, `theme`, …) |

Modes are the design file's `appearance` axis: `light`, `dark`, `hc-light`, `hc-dark` (DESIGN-GAPS records that the
guide calls it `Mode`). The five ungrouped `vellum-*` variables stay in `tokens.json` and are not generated.

## Regenerate (after `vellum.pen` changes)

Regenerate the inventory first (`docs/runbooks/design-mcp.md`), because the token export checks it names the
same variables. Then, with the pen CLI signed in:

```text
pnpm design:open --no-open
pnpm design:tokens
```

From an agent session with the Pencil MCP (rules in `design-mcp.md`):

1. `node scripts/design-tokens.mjs --print-query`; run it with `execute` on the read-only copy; save the output.
2. `node scripts/design-tokens.mjs --print-export --staging .design-cache/tokens-staging`; run both lines.
3. `node scripts/design-tokens.mjs --raw <saved output> --staging .design-cache/tokens-staging`.

The query and the export only read (Get, GetVariables, Print, Export). Output is deterministic.

## Checks

| Command | Where | Fails when |
|---|---|---|
| `pnpm tokens:check` | CI | `tokens.json` was made from another `vellum.pen` than the inventory; the generated files differ from `tokens.json`; DESIGN.md and the design differ in a way no `check=token-…` line in DESIGN-GAPS.md accepts |
| `pnpm tokens:lint` (also in `pnpm lint`) | CI | a text pair is under 4.5:1 or an icon, focus or canvas pair under 3:1 in any appearance; a colour literal is in UI code; selection blue or component violet is on a button; an agent hue is outside a marker, cursor, lease, placeholder or session dot |
| `node scripts/token-page-compare.mjs` | local (`VELLUM_LANES=chrome,design`) | the Storybook token page differs from the Tokens sheet in more than 1 % of pixels |

### Resolving a DESIGN.md gap

`tokens:check` stays red until a person decides each difference. Either change `vellum.pen` or DESIGN.md (both
are read-only to agents), or accept the design file's side by adding a new line to DESIGN-GAPS.md that carries
the key the check prints, for example `check=token-value:chrome/tertiary`. The gap keys are:
`token-value:<token>`, `token-missing:<token or type/role>`, `token-extra:<token>`.

### The token page comparison

The page (`packages/ui-tokens/src/token-page.ts`, story `Foundations/Tokens`) is rebuilt from the Tokens sheet's
node tree: same geometry, every colour and number from the tokens. The comparison renders it in headless Chrome
with Inter (`crates/adapters/text-parley/fonts/`) and JetBrains Mono (`packages/ui-tokens/fonts/`), and compares
with `comparePixels`: a pixel differs when a premultiplied channel is more than 48 apart, and at most 1 % may
differ (DESIGN-DECISIONS.md). Output: `target/token-page/{actual,diff}.png`. `--boxes` prints element boxes, for
comparing with the sheet's node bounds.

What is known to differ: pen.dev draws text with its own rasteriser, and appears to draw the non-hundred weights
450 and 650 as 400 (the sheet's 650 title renders at regular weight; drawing the page at 400 lowers the
difference from 0.94 % to 0.73 %). The page keeps the specified weights. Strokes in the design are centred on the edge, so the page draws them as half-pixel inset and outset
shadows.

The PNG is gitignored like every snapshot, so CI cannot run the comparison.

## Storybook

`packages/ui-tokens` has Storybook 10.6 (html renderer, Vite), set up with `npm create storybook@latest` and then
pinned in the catalog (version-baseline C24). The token page is the story `Foundations/Tokens`.

| Command (in `packages/ui-tokens`) | Does |
|---|---|
| `pnpm storybook` | dev server on port 6006 |
| `pnpm build-storybook` | static build in `storybook-static/` (gitignored) |
| `pnpm test` | unit tests only (Vitest project `unit`); what `pnpm test` at the root runs |
| `pnpm test:storybook` | renders every story in headless Chromium (Vitest project `storybook`); needs Playwright's Chromium (`pnpm exec playwright install chromium`) |

`.storybook/preview.ts` loads the generated `tokens.css` and the two UI faces (`.storybook/fonts.css`, served by
`staticDirs`). Telemetry is off in `.storybook/main.ts`; also set `STORYBOOK_DISABLE_TELEMETRY=1` when running the
CLI. Prompt 45's visual harness (a story per component, variant and theme) is a separate, React setup.
