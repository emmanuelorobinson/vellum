# Operation fixtures

TEST DATA only. Canonical `.vellum` documents for Prompt 12's core tests; no renderer is required.

- `scale.vellum`: proportional content, text, variable-bound lengths, per-side strokes, dash lengths,
  corner radii, padding and shadow lengths. The test supplies the text's measured 80 × 30 box and the
  resolved number variable value, 16. A twofold scale fixes Fill/Hug dimensions and preserves colour links.
- `slots.vellum`: an empty frame inside a component with a suggested component. The instance fills it
  with an ordinary rectangle, demonstrating that suggestions never restrict content. Detach keeps
  the content and removes component-only slot markers.

Tests replay the generated edits and their exact inverses. Regenerate canonical bytes with:

```sh
cargo run -p vellum-codec-json --example canonicalize -- fixtures/ops/scale.vellum fixtures/ops/slots.vellum
```
