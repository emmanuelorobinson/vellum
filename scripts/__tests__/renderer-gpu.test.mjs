// needs: Chrome hardware WebGL2 + built emscripten renderer + generated native fixtures.
import { test } from "node:test";
import { runRendererGpu } from "../renderer-gpu.mjs";

test("production editor/native parity, context recovery and Renderer contract (gpu lane)", {
  skip:
    !(process.env.VELLUM_LANES ?? "").split(",").includes("gpu") &&
    "gpu lane not enabled; see docs/runbooks/renderer.md",
}, async () => {
  await runRendererGpu();
});
