# Figma import and image work (Prompt 66)

The daemon/CLI implement REST import, image generation and local image operations. Every completed canvas
change goes through one attributed core transaction. No account is required. Native editor authoring still
needs Prompt 55's writable ABI; the desktop's Figma entry explicitly says unavailable until that is attached.
C25 provides host ports for configured providers/models and native keychain controls; there is no key field
in editor preferences or the document. Hosts must persist provider/model choices to native settings and
supply the AuthoringHost/FigmaImportHost callbacks before enabling the controls.

## Native configuration

`vellum asset-settings` shows/saves the non-secret settings in `image-settings.json` alongside daemon state.
Configure a provider by supplying all three values together. This adapter expects the GPT Image-compatible
JSON protocol, returning one base64 PNG. Choose the model offered by your provider; no model is hardcoded.
For example (replace MODEL_ID):

```sh
vellum asset-settings --provider openai --endpoint https://api.openai.com/v1/images/generations --model MODEL_ID
vellum asset-settings --segmentation-model /absolute/path/to/reviewed-model.onnx
vellum daemon stop
```

The next daemon starts with those settings. This does not download a model, create a keychain entry or
contact a provider. An invalid configuration fails explicitly; remove/fix its JSON to recover. Provider
configuration can also be supplied by a composition root through Documents' typed setters.

Use `vellum asset-key figma` or `vellum asset-key image` with stdin piped from a secret manager. Terminal
input is refused to avoid echoing credentials. Never pass a credential as an argument or save it in JSON,
logs or a shell history. The commands save to the OS keychain (`figma_token`, `image_model`) under the
existing Vellum service. They return only `stored: true`. Real-keychain acceptance has not been run here.

## Tools

Open a document to establish its session before requesting outside access. The ordinary tool/CLI/MCP
registry exposes these additive inputs:

- `import {figma: <Figma design URL or key>, parent: <page/frame id>}` retrieves public REST JSON and image
  fills. Each actual API/CDN origin needs its own PermissionBroker decision before I/O. No approval input
  is accepted. Recorded REST JSON may also use `text`/`data` with `format: "figma-api"` without fetching.
- `fill_image {node, prompt, model?}` uses the configured image provider; `model` defaults to native settings.
  Its provider origin needs permission. Result bytes are stored locally, and metadata.generated records
  provider/model. Prompt text and raw provider payloads are not persisted in metadata or step previews.
- `image_ops {node, action: "remove_background" | "trace"}` uses stored image bytes without network access.
  Select an image layer or a leaf shape with exactly one image fill. Background removal preserves source
  alpha and all other layer fields. Trace requires fixed numeric dimensions, replaces the layer with editable
  vectors and preserves its id, placement, size and metadata. It reports that fit/crop/decorations are lost.

Traces accept at most 1 megapixel; local image decoding accepts at most 16 megapixels, 8192 pixels per side
and 32 MB of encoded input. Figma imports are bounded by bytes, depth, layers, image count and total asset
bytes. HTTP is HTTPS-only on public addresses, DNS-pinned, with no redirects and bounded body/time. API keys
are header-only; Figma tokens never accompany CDN downloads. Provider errors are payload-free.

Figma conversion is intentionally partial: editable frames/shapes/text/path geometry, solid/image fills,
base text styles and basic strokes. Auto layout/component links are flattened; gradients, masks, effects,
prototype interactions, independent radii, image transforms/filters, mixed text and other unsupported
features are reported. Fonts are retained by family but not downloaded. There is no proprietary `.fig`
parser. This is not a full-fidelity Figma conversion claim.

## Local model and acceptance lanes

The stable tract CPU backend consumes an explicitly installed U2Net-style ONNX model with input
`[1,3,320,320]` float RGB, ImageNet mean/std normalization, and first output `[1,1,320,320]` float foreground.
The output is normalized and resized to the original pixels. Provision a reviewed, appropriately licensed
model separately. Incompatible models fail; arbitrary ONNX architectures are not supported. The 256 MB
model cap bounds input bytes, not graph complexity or inference cost. Models are trusted local configuration.

```sh
cargo test -p vellum-import-figma-api -p vellum-image-ops -p vellum-images-generate --lib --tests
cargo test -p vellum-contracts --test media
node scripts/skia-build.mjs test -p vellum --test image_work --test asset_tools --test mcp
# Native-model lane: Python/onnx creates a tiny fixture, Rust performs the actual inference.
python -m pip install onnx==1.19.1
python scripts/image-model-fixture.py target/image-model-fixture/segmentation.onnx
VELLUM_TEST_ONNX_MODEL="$PWD/target/image-model-fixture/segmentation.onnx" cargo test -p vellum-image-ops --test local native_onnx_inference -- --ignored
# Network lane: disposable loopback servers, no external service or real credential.
cargo test -p vellum-model-http --test fetch -- --include-ignored
pnpm --filter @vellum/editor-react exec playwright test --config e2e/playwright.config.ts images.spec.ts --workers=1
pnpm checksum
```

The tiny ONNX fixture verifies inference wiring and alpha application, not real segmentation quality.
The browser fixture uses production components over scripted hosts; it is not native editor write acceptance.
Goldens keep the unchanged 1% threshold and write `target/screen-goldens/p66-*`. No tolerance/reference was
changed. The Figma report title differs intentionally from the `.fig` example in the design; the gap is logged.
Live Figma/GPT Image requests, real keychain, production-model quality/performance, Stop responsiveness during
synchronous image work, remote CI and Windows/Linux remain unverified. The HTTP deadline does not make a
synchronous operation cancellable. Closing a generation panel prevents its late result from being applied.

Dependency advisory checks remain failures, including deprecated adler/ansi_term/atty pulled by stable
vtracer 0.6.5. No advisory exception or prerelease dependency was added. See ADR-029 and reports.md for the
exact passing and failing commands.
