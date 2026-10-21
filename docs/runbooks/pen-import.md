# Importing .pen files

The `import` tool accepts pen.dev `.pen` version 2.21 through its ordinary `path`, `text` or base64 `data`
inputs. File names and content are detected automatically; `format: "pen"` selects it explicitly. This is
an independent reader of the public format, with legal review confirmed by the owner (ADR-030).

With a document open, give `parent` as the destination page/frame id. For example, the CLI calls the same
registry used by MCP:

```sh
vellum tool import --document /absolute/path/design.vellum --parent PAGE_ID --path /absolute/path/source.pen
```

The source file stays unchanged. Imported layers receive new Vellum ids and one attributed, undoable
transaction. The result lists created ids, layer/frame counts, stored asset hashes and conversion notes.
Save the destination through the normal save workflow. `.vellum` remains the native document format.

Supported content includes frames/flex layout, groups, rectangles, ellipses, polygons, vector paths,
text, registered icons as editable vectors, solids, gradients, meshes, strokes, shadows/blur and image fills. Variables are resolved
under inherited/default themes. Local component references, descendant overrides and child replacements
expand into editable layers; component links, slots and variable bindings are not retained. Colors,
sizes and text values remain editable after import. Fonts are named but font files are not copied.

Each companion PNG/JPEG/WebP is a separate permission-brokered read relative to the source file. Missing,
denied or invalid images are omitted with notes. Inline PNG/JPEG/WebP base64 images are supported after
existing image-header validation. An inline document never triggers implicit local file reads. Remote,
absolute, parent-traversal, encoded and non-file image URLs are not fetched. Symlinks are subject to the
existing broker's canonical-target permission checks. No network is needed for this importer.

Scripts and live browser nodes are omitted; shader fills/effects and external design libraries are not
loaded. Source notes/prompts/context objects become inert text. Foreign metadata and hyperlinks are
omitted, with conversion notes. Importing a file cannot start an agent, run code or navigate a browser.

This is a partial editable conversion, not a pixel-fidelity or round-trip claim. Check gradient direction
and extent and the placement of rotated layers; the destination uses a different rotation pivot. Paths
without a view box reuse Vellum's existing SVG bound sampling. Unsupported features are reported. Unknown
schema versions are refused; for older files, save a separate 2.21 copy in pen.dev and import that copy.

The reader accepts documented JSON comments. Limits: 32 MB input, 20,000 source/expanded layers, depth 64,
32 reference/variable aliases, 1 MB per path, 64 MB expansion work/properties and 128/64 MB image assets. Malformed, duplicate-id,
cyclic and over-budget content fails explicitly before document changes. Native Ops remain the final validator.
The installed desktop still needs Prompt 55's writable runtime before any native editor import can commit;
this addition provides the working daemon/CLI/MCP path without pretending that runtime is available.

## Verification

```sh
cargo test -p vellum-import-pen --lib --tests
node scripts/skia-build.mjs test -p vellum --test pen_import --test asset_tools --test mcp -p vellum-import-svg
node scripts/skia-build.mjs clippy -p vellum -p vellum-import-pen -p vellum-import-svg --all-targets -- -D warnings
pnpm type-check
pnpm deps:check
pnpm clean-room
pnpm checksum
```

Tests use independently authored `TEST DATA` inputs, not a vendor file or Vellum's protected design.
They cover themes/layout, multi-paint/per-side strokes, component expansion/replacements/cycles, inert
active content, asset authorization and omission, inline images, bounds, byte amplification and undo.
No native GUI or general import-fidelity golden is claimed. See reports.md for exact results.

Public format reference: [pen.dev format documentation](https://docs.pen.dev/for-developers/the-pen-format).
