# Layout oracle runbook

The Chrome layout oracle (guide Prompt 6, ADR-002) turns small HTML fixtures written in Vellum's layout semantics into
expected boxes computed by headless Chrome. Engines compare against those boxes at 0.5 px (Invariant 7).

## Files

| Path | What it is |
|---|---|
| `fixtures/layout-oracle/<name>.html` | a fixture: Vellum properties as `data-*` attributes on `<div>`s, nothing else |
| `fixtures/layout-oracle/expected/<name>.json` | Chrome's boxes for it, with the props, written by the generator |
| `fixtures/layout-oracle/expected/manifest.json` | the Chrome version that wrote the boxes, and the fixture list |
| `scripts/lib/layout-oracle.mjs` | fixture parser, semantic resolution (hug and fill fallbacks), CSS mapping |
| `scripts/gen-layout-oracle.mjs` | the generator (`pnpm layout-oracle`) |
| `crates/vellum-testkit/src/layout_oracle.rs` | Rust loader and `compare` for engines |
| `crates/adapters/layout-taffy/` | production LayoutEngine, contract and 150+ oracle fixture tests |
| `scripts/fixtures-layout.mjs` | reproducible TEST DATA matrix inputs; Chrome writes expectations |
| `spikes/layout/` | historical spike entry point, now delegates to production |

## Needs

Chrome. The generator uses `VELLUM_CHROME` if set, else `/Applications/Google Chrome.app/...` on macOS or
`google-chrome` / `chromium` on Linux. It talks to Chrome over `--remote-debugging-pipe`: no TCP port, no npm packages.
CI's `layout-oracle` job installs Chrome for Testing 155.0.8059.39 (the version in `manifest.json`).

## Commands

```sh
pnpm layout-oracle                      # regenerate every expected file and the manifest
pnpm layout-oracle --check              # regenerate in memory and compare (CI does this); exit 1 on a difference
pnpm layout-oracle --only row_gap       # regenerate one fixture
pnpm layout-oracle --check --out /tmp/o # compare, and keep the regenerated files to diff by hand
VELLUM_LANES=chrome pnpm test:scripts   # the script tests, including the chrome-lane --check test
cargo test -p vellum-layout-taffy       # production contract, incremental tests and committed boxes
```

## Adding a fixture

1. Copy an existing fixture. Keep the `TEST DATA` comment and give it a `<title>` that says what it shows.
2. Every node needs `data-width` and `data-height` (a number, `hug` or `fill`). Optional: `data-layout`
   (`none` `row` `column` `grid`), `data-gap`, `data-padding` (1, 2 or 4 numbers), `data-justify`
   (`start` `center` `end` `space-between` `space-around`), `data-align` (`start` `center` `end`), `data-columns`,
   `data-rows` (grid only), `data-width-fallback`, `data-height-fallback`, `data-position="absolute"`, `data-x`,
   `data-y`, `data-visible="false"`, `data-clip="true"`, `data-layout-includes-stroke`, `data-stroke` (1, 2 or 4 widths),
   `data-stroke-align` (`inside`, `center`, `outside`). Anything else, including `style`, is rejected.
3. Run `pnpm layout-oracle --only <name>`, then `pnpm layout-oracle` once so the manifest lists it.
4. Open the expected JSON and check the boxes are what a designer would expect. If Chrome disagrees with the intended
   semantics, the mapping in `scripts/lib/layout-oracle.mjs` is wrong or the semantics need a decision: record it in
   ADR-002, do not edit the expected file.
5. Run `cargo test -p vellum-layout-taffy`. Every fixture must match; no mismatch exemptions. Record a new
   semantic choice in an ADR, and regenerate expectations only with Chrome.

The script tests fail if a fixture's props no longer match its expected file, so editing a fixture without
regenerating is caught.


## Prompt 18 semantics (ADR-012)

The suite has 186 fixtures. The matrix adds combinations of flex justification/alignment, mixed fill/fixed sizing,
fractional gaps/padding, equal grid tracks, nesting, hidden subtrees and included/ignored strokes. Run
`node scripts/fixtures-layout.mjs` to reproduce its inputs, then `pnpm layout-oracle` to regenerate boxes.

The stroke switch reserves the frame's own inward stroke alongside padding (inside full, center half, outside zero).
Fixed sizes stay border-box sizes; absolute child x/y stay relative to the outer edge. Chrome snaps border widths:
the CSS mapping uses an integer border and puts its fractional remainder in padding, preserving the exact sum.
Text measurement is tested through the injected TextEngine port; the browser corpus has no font-dependent text yet.
