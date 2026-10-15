# ADR-013: Text layout and explicit font inputs

Status: implemented in Prompt 19; port additions and sizing policy await architecture synchronization.

Parley 0.12.0 shapes, resolves bidi, breaks lines and justifies. Renderers consume glyphs, never
shape again (ADR-001). Enable `std` and `complex-scripts`, disable default/system features.
The latter adds ICU segmentation models so multilingual text does not emit missing-model errors.
The core and file format remain independent of Parley.

## Inputs and output

- `RichText` retains underline, strike, link, vertical alignment and resizing from the resolved node.
  `layout(text, width)` and `measure` retain their original, explicitly constrained content contract.
  `layout_box` applies resizing: auto-width ignores the proposed width (hard breaks remain), auto-height
  wraps at it and grows the height, fixed retains both dimensions. In resolved layout-tree conversion,
  auto-width makes both dimensions hug, auto-height makes only height hug. Fixed uses the document sizes.
- Fixed-box vertical alignment uses spare height only. Overflow begins at the top, keeps every line and
  reports x/y overflow; later display-list/clip handling decides what is visible. Left/right alignment is
  physical; justify leaves a paragraph's last line direction-relative start-aligned, as in CSS.
- Omitted line height uses the selected font's metrics. Explicit heights are pixels. No quantization in
  shaping. Model outputs are f64 converted from Parley's f32; line width excludes hanging whitespace and
  includes justification. Parley's empty-text placeholder is removed from ranges, width and glyphs.
- `Glyph.y` is relative to the line baseline, `Glyph.x` to the line origin. Logical UTF-8 cluster offsets
  survive visual bidi order and ligatures. Glyph runs carry original run index, link data, decorations
  from font metrics, normalized variation coordinates (F2Dot14) and synthetic bold/italic instructions.
- `FontSource::data` supplies shared immutable SFNT bytes and a collection face index by handle. Its
  default returns none for fakes. Handles belong to one registry, stay readable after registration and
  are never serialized into the document. No vendor font or platform object crosses the port.

## Fonts

A named family is selected from document fonts, then the Google Fonts cache, then system fonts, then
bundled Inter. The first tier containing the family owns CSS weight/style matching (including nearest
weight), even when a later tier has an exact face. Tier-private aliases prevent Fontique from mixing
same-named families across tiers. Registration order cannot reverse tier priority.

Hosts inject decoded TTF/OTF bytes tagged with their tier. The adapter does not enumerate the machine,
read files, install fonts, access a Google service, or create a network request. Prompt 24 supplies disk
and WOFF/WOFF2 decoding; hosts supply platform font discovery. Multi-family collections are explicitly
refused; single-family collections retain their face indices. Same-tier later registrations replace
matching face attributes for future layouts; old handles keep their bytes.

`resolve` remains strict (unknown family is none, per its contract). Shaping substitutes bundled Inter
and reports requested family, substitute and UTF-8 usage range. Coverage fallback tries supplied families
in tier/registration order and then Inter; uncovered clusters report `MissingGlyph`, never silent success.
Bundled Inter is the unmodified pinned Google Fonts asset with its OFL license and hash recorded beside it.
Sharing the registry and shaping contexts reuses font caches; completed paragraph layouts are not cached.
Callers mark affected text dirty in LayoutEngine after changing a font source, as recorded in Prompt 18.

## Reference study (no source ported)

Parley's layout exposes visual runs, logical clusters, positioned glyphs and font-derived decorations;
Fontique separates CSS face matching, script fallbacks and source-byte caching. Vellum keeps that division
behind pure-data ports and explicit, ordered font bytes for reproducibility across hosts.
Penpot's text document describes dynamic font bytes and Unicode fallback, but shapes with SkParagraph;
Vellum keeps Parley as its sole layout engine. No Penpot source or reference implementation was copied.
Sibling projects were searched: Clarum has Inter assets but no reusable Rust font/layout implementation.

## Evidence and limits

The shared TextEngine suite and rich-style/box extension run on Parley and the fixed-metric fake.
Thirty Prompt 5 fixtures compare glyph ids, selected font, line count and absolute x/y at 0.001 px;
separate checks cover RTL logical offsets, CJK breaks and emoji ZWJ/modifier clusters. They run in a
fonts CI lane after checksum-verified downloads; ordinary tests use only bundled Inter. The oracle is
produced by the original spike's text path, not this adapter. No renderer pixels or large-document
performance were measured in this prompt. Pixel parity remains Prompt 22/23's responsibility.
