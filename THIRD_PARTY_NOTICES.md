# Third-party notices

Code ported into this repository from other projects. Only MIT and Apache-2.0 sources may be ported,
each in a file whose header names the source repo, path and commit (`ported-from:`). Nothing is ever
ported from Penpot (MPL-2.0) or tldraw (commercial licence), and nothing is copied from pen.dev.

Dependencies installed through Cargo and pnpm are covered by their own licences and the lockfiles; this
table is for copied or adapted source only.

| Project | Source repository | Path | Commit | Licence | Vellum file |
|---|---|---|---|---|---|

## Bundled font asset

`crates/adapters/text-parley/fonts/InterVariable.ttf` is unmodified Inter from google/fonts,
commit `0b58fb370093f9a9f4ff785d94405710b79de67c`, `ofl/inter/Inter[opsz,wght].ttf`, renamed only.
SIL Open Font License 1.1; copyright and license are reproduced in the adjacent `OFL.txt`.
The adjacent README records the source URL and SHA-256. This is a licensed font asset, not ported code.

`crates/adapters/text-parley/fonts/NotoColorEmoji.ttf` is unmodified Noto Color Emoji from
googlefonts/noto-emoji commit `e20cbc2bbec1926686be9f9bee7d1d2cfa1fea0e/2D`, `fonts/NotoColorEmoji.ttf`.
SIL Open Font License 1.1; copyright/license in the adjacent `NotoColorEmoji.OFL.txt`.
This is a licensed font asset, not ported code.

## Native graphics libraries (Prompt 22)

Skia (BSD-3-Clause) and its embedded FreeType (FTL or GPL-2.0; distributed under FTL) are built from
the source revisions pinned by skia-bindings 0.153.3. Their original notices ship with release
artifacts; `scripts/skia-notices.mjs` assembles them from the synchronized source tree.
HarfBuzz/ICU are disabled: Parley supplies positioned glyphs and Skia does no text layout.

## Web font test fixtures

`crates/adapters/text-parley/tests/fonts/inter-latin-400-normal.woff` and `.woff2` are unmodified Inter Latin 400
files from the npm package `@fontsource/inter` 5.3.0, fetched from jsDelivr at that version. SIL Open Font License
1.1; copyright and license are in the adjacent `OFL.txt`, and the adjacent README records the URLs and SHA-256.
They are test inputs only and are not shipped. These are licensed font files, not ported code.

## TLS root certificate data (Prompt 32)

`webpki-root-certs` 1.0.9 (via reqwest/rustls-platform-verifier) includes root-certificate data under
CDLA-Permissive-2.0. The agreement is preserved in
`crates/adapters/model-http/notices/CDLA-Permissive-2.0.txt` and must accompany redistributed data.
This is dependency data, not ported project code. Cargo-deny permits only this exact package/version.
