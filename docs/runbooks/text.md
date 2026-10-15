# Text layout tests

Ordinary tests are offline and independent of installed fonts:

```sh
cargo test -p vellum-text-parley
cargo test -p vellum-core --test layout
```

The **fonts lane** needs the five Prompt 5 Noto fonts (about 25 MB, network once). The fetch script
checks the pinned SHA-256 for each file. No GPU or Chrome is required. A missing font fails the lane:

```sh
node spikes/renderer/scripts/fetch-fonts.mjs
node scripts/gen-text-fixtures.mjs --check
cargo test -p vellum-text-parley --test fonts_lane -- --ignored
```

The tests are marked ignored with a `fonts lane` reason during ordinary `cargo test --workspace`.
CI has a separate `text-fixtures` job which downloads verified fonts and explicitly runs them.
To regenerate the committed TEST DATA oracle from the original Prompt 5 spike (Parley 0.12.0):

```sh
node scripts/gen-text-fixtures.mjs
```

The fixture records glyph ids, font-stack indices, size, absolute positions and line counts.
Comparison tolerance is 0.001 px per position; glyph identity, selected font and line counts are exact.
The old spike can print ICU missing-segmentation-model notices; its original layout stays unchanged.
The production adapter enables Parley's `complex-scripts` data and has no such notices.

Core types carry the resolved rich-text inputs and shared immutable font bytes. `ParleyFonts::new()`
provides bundled Inter; register document, cached Google and system SFNT bytes with `register` before
laying out. Font discovery, filesystem persistence and WOFF/WOFF2 decoding belong to hosts/Prompt 24.
After registration mark affected text dirty (or `DirtySet::all()`) before invoking incremental layout.
Reuse the registry for rendering so its FontHandle values identify exactly the bytes that shaped text.
See ADR-013 for box dimensions, overflow, fallback diagnostics and the renderer data contract.
