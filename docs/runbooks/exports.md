# Export backends (Prompt 41)

`export` returns an in-memory bundle of `{name, mime, base64}` files, warnings and optional clipboard MIME data.
It writes no files. Agent callers that save files still need the PermissionBroker. The editor consumes
`ExportBundle::clipboard()` for Copy as PNG / Copy HTML; native clipboard wiring belongs to the hosts.

Formats: `png`, `jpeg`, `webp`, `svg`, `pdf`, `html-css`, `html-tailwind`. `nodes` preserves selection order.
PNG/JPEG/WEBP accept `scale: 1|2|3`. PDF has one page per selected node, in selection order; PDF/SVG use canvas
units. Multiple raster selections return a stored ZIP with a fixed timestamp and unique safe filenames.
`suffix` changes suggested image names. JPEG composites transparent areas on white at quality 95; WEBP uses
quality 100 and preserves alpha. SVG uses the full renderer's paths and embedded images.

HTML options: `scaffold` (default true), `assets: inline|files` (default inline). A fragment includes its own
style element. Text image layers retain the current canvas resolution. Both formats include compiled CSS, without a runtime or network request. Tailwind uses v4
arbitrary-property utilities; it requires no CDN or build to preview. Supported solid shapes and text are DOM
with the resolved canvas layout, fixed to its current dimensions. Complex appearance (gradients, images,
effects, paths, styled text, collection fonts) uses an SVG asset of the selected frame (PNG when it includes text), with a warning.
Text has a visible PNG layer from the shared renderer, with the original font-backed DOM text kept
transparent for accessibility and editing. This is reported in warnings. Font bytes and text image assets
are included as data URLs or bundle files. Bundled fonts are represented by the rendered text layer, so an ordinary
text export fits the synchronous tool budget; editing that text with its original font requires that font
to be installed. Document and library font files accompany their editable text. This is a visual code export, with no
inferred responsive breakpoints, app interactions, component framework or semantics. `get_spec` remains the
input for an agent writing application code.

Exports derive only the selected subtrees from the current snapshot, including unsaved live state. Each
selected subtree is independent of its siblings and rebased to its own origin. The composition root opens a
fresh document font collection and resolves libraries through the rooted filesystem. Hash-verified image
bytes come only from canvas asset references. Missing fonts, images, libraries and stale script output fail
explicitly. Export does not run scripts or fetch assets. A library in directory format still needs the future
shared directory-library loader. Ordinary screenshot services retain the earlier resource limitations.

Limits: 100 selections, 8192 px per side, 16.7 megapixels per frame, 67.1 megapixels per selection, 32 MiB per
bundle. The synchronous tool has a smaller result budget; native callers can consume the bundle directly.

Verification:

```sh
node scripts/skia-build.mjs test --offline -p vellum --test exports
node scripts/skia-build.mjs test --offline -p vellum-export-html -p vellum-export-raster
node scripts/skia-build.mjs run --offline -p vellum --example export-goldens -- --check
node scripts/export-goldens.mjs
pnpm checksum
```

The last script needs Chrome (`VELLUM_CHROME`, or the normal local installation); it is the tagged `chrome`
lane. It compares 32 HTML cases (both formats, scaffold/fragment, inline/file assets) against committed native
PNG goldens for geometry, auto-layout, text, and gradient fallback. It uses CDP pipes, blocks HTTP(S), and
inlines bundle-owned resources for the comparison. Generated HTML and browser screenshots stay under
`target/export-goldens/`. Regenerate PNG goldens with the example without `--check`, then review the images.
