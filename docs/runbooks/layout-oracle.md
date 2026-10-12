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
| `spikes/layout/` | Taffy computing every fixture (`cargo test` there) |

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
cargo test --manifest-path spikes/layout/Cargo.toml   # Taffy against the committed boxes (no Chrome needed)
```

## Adding a fixture

1. Copy an existing fixture. Keep the `TEST DATA` comment and give it a `<title>` that says what it shows.
2. Every node needs `data-width` and `data-height` (a number, `hug` or `fill`). Optional: `data-layout`
   (`none` `row` `column` `grid`), `data-gap`, `data-padding` (1, 2 or 4 numbers), `data-justify`
   (`start` `center` `end` `space-between` `space-around`), `data-align` (`start` `center` `end`), `data-columns`,
   `data-rows` (grid only), `data-width-fallback`, `data-height-fallback`, `data-position="absolute"`, `data-x`,
   `data-y`, `data-visible="false"`, `data-clip="true"`. Anything else, including `style`, is rejected.
3. Run `pnpm layout-oracle --only <name>`, then `pnpm layout-oracle` once so the manifest lists it.
4. Open the expected JSON and check the boxes are what a designer would expect. If Chrome disagrees with the intended
   semantics, the mapping in `scripts/lib/layout-oracle.mjs` is wrong or the semantics need a decision: record it in
   ADR-002, do not edit the expected file.
5. Run `cargo test` in `spikes/layout`. A new Taffy mismatch goes into `spikes/layout/known-mismatches.txt` with a
   reason, and into ADR-002.

The script tests fail if a fixture's props no longer match its expected file, so editing a fixture without
regenerating is caught.
