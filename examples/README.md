# Examples

All four examples are original Vellum examples, available without an account or network.
`catalog.json` names their files and shader defaults. Shader sources use the bounded `.vsh`
dialect documented in `docs/runbooks/shaders.md`; arbitrary GLSL is not accepted.

The gallery host copies shader sources into the open document's dependency folder before returning
a relative source path. Applying that choice is one ordinary person-origin Op transaction. Scripts
open as script nodes and run only in ScriptRuntime; libraries open as documents or attach through
the existing library import path. Browsing or previewing never changes the current document.

`goldens/*.rgba` are 16 × 16 unpremultiplied RGBA analytic references, top row first, one per example.
They are authored from the examples' geometry and colours, not regenerated from the renderer under test.
The native example suite runs the actual shader, script and variable resolver pipelines and compares
all pixels. The shader examples also run through the browser WASM renderer in the shader lane.
