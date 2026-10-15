# Bundled Inter

Unmodified `ofl/inter/Inter[opsz,wght].ttf` from
[google/fonts, commit 0b58fb370093f9a9f4ff785d94405710b79de67c](https://github.com/google/fonts/tree/0b58fb370093f9a9f4ff785d94405710b79de67c/ofl/inter),
renamed `InterVariable.ttf`. Family: Inter; variable optical size and weight; roman.
Licensed under SIL Open Font License 1.1, reproduced in `OFL.txt` (trailing whitespace normalized).

SHA-256:

- `InterVariable.ttf`: `29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031`
- `OFL.txt`: `5dd548d31a85f756e01d63e00d7faf1e324103ed3e9102fcbbabf2cc2db6dd39`

Embedded in `vellum-text-parley` for an offline fallback on every target. Rendering uses the
exact bytes, face index, normalized axes and synthesis instructions returned with the glyphs.
The family named Inter in a document may override it; missing-family substitution always starts
with this bundled face. Other Unicode coverage comes from explicitly supplied fonts and is reported
when missing. No font is downloaded or installed by the adapter.

Noto Color Emoji is bundled for deterministic offline colour emoji fallback (Prompt 22).
Unmodified `fonts/NotoColorEmoji.ttf` from googlefonts/noto-emoji,
commit `e20cbc2bbec1926686be9f9bee7d1d2cfa1fea0e/2D`.
SHA-256: `15671215ab769fdc7162a045d56fd7d7e477c51b04e6b3c761d914d8fdd6cc44`.
Source: https://raw.githubusercontent.com/googlefonts/noto-emoji/e20cbc2bbec1926686be9f9bee7d1d2cfa1fea0e/2D/fonts/NotoColorEmoji.ttf
License: SIL OFL 1.1, reproduced in `NotoColorEmoji.OFL.txt`.
The renderer only receives these bytes through FontSource; it never substitutes a platform font.
