// needs: Chrome, built editor WASM, generated native fixtures. Enabled by the gpu lane only.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";

const root = resolve(import.meta.dirname, "../..");
test("every committed renderer golden has explicit tolerances and complete RGBA", () => {
  const directory = resolve(root, "fixtures/rendering"),
    tolerances = JSON.parse(readFileSync(resolve(directory, "tolerances.json")));
  assert.deepEqual(
    readdirSync(directory)
      .filter((n) => n.endsWith(".rgba"))
      .map((n) => n.replace(".rgba", ""))
      .sort(),
    Object.keys(tolerances).sort(),
  );
  for (const [name, renderers] of Object.entries(tolerances)) {
    assert.equal(statSync(resolve(directory, `${name}.rgba`)).size, 256 * 256 * 4);
    for (const renderer of ["cpu", "gpu", "basic"]) {
      const t = renderers[renderer];
      assert.ok(
        t.channel >= 0 && t.channel <= 4 && t.fraction >= 0 && t.fraction <= 0.02 && t.p95 <= 3,
      );
    }
  }
});
test("fixture cross-product, direct editor/headless comparison, and Renderer contract (gpu lane)", {
  skip:
    !(process.env.VELLUM_LANES ?? "").split(",").includes("gpu") &&
    "gpu lane not enabled; see docs/runbooks/renderer.md",
}, () => {
  const r = spawnSync(process.execPath, ["scripts/renderer-goldens.mjs"], {
    cwd: root,
    encoding: "utf8",
  });
  assert.equal(r.status, 0, r.stderr + r.stdout);
});
