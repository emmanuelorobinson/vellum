// Tagged `gpu` (guide §11 rule 19). Needs: Chrome with a working GPU (VELLUM_CHROME or the default
// install path), the spike build in web/dist and target/release (`bash spikes/renderer/scripts/build.sh`,
// which needs emsdk 4.0.6 and ninja). Runs only when VELLUM_LANES contains "gpu"; otherwise it is
// reported as skipped, never as passed.
//
// Fixture #1 is drawn by the native skia-safe path, by Renderer A (CanvasKit in TS) and by Renderer B
// (skia-safe in WASM over WebGL2). Each browser capture must match the native PNG within tolerance,
// compared by vellum-testkit's pixel diff (via `spike-skia-native compare`).

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { chromePath, dataUrlToBuffer, launchChrome, openSpike } from "../lib/chrome.mjs";
import { startServer } from "../server.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const spike = resolve(here, "../..");
const native = join(spike, "target/release/spike-skia-native");
const out = join(spike, "artifacts/gpu-test");
const lanes = (process.env.VELLUM_LANES ?? "").split(",").map((s) => s.trim());

function skipReason() {
  if (!lanes.includes("gpu")) return "gpu lane not enabled (set VELLUM_LANES=gpu)";
  if (!chromePath()) return "Chrome not found (set VELLUM_CHROME)";
  return null;
}

// Fixture #1 is an axis-aligned rect on whole pixels: every path should agree almost exactly.
const TOLERANCE = { channel: 2, fraction: 0 };

test("fixture #1 renders the same natively and through both browser paths", {
  skip: skipReason() ?? false,
}, async () => {
  for (const file of [
    native,
    join(spike, "web/dist/spike_scene_wasm.wasm"),
    join(spike, "web/dist/skia_wasm.wasm"),
  ]) {
    assert.ok(existsSync(file), `${file} is missing; run spikes/renderer/scripts/build.sh`);
  }
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const nativeRun = spawnSync(native, ["fixtures", out], { encoding: "utf8" });
  assert.equal(nativeRun.status, 0, nativeRun.stderr);

  const { server, url } = await startServer();
  const chrome = await launchChrome();
  const pairs = [];
  try {
    for (const renderer of ["a", "b"]) {
      const info = await openSpike(chrome.cdp, `${url}?renderer=${renderer}&fixture=1`);
      assert.equal(info.width, 256);
      assert.doesNotMatch(
        info.glRenderer,
        /SwiftShader|llvmpipe/i,
        "the gpu lane needs a hardware GPU",
      );
      const png = dataUrlToBuffer(
        await chrome.cdp.evaluate("window.__spike.then(s => s.captureDataUrl())"),
      );
      const actual = join(out, `fixture-01.${renderer}.png`);
      writeFileSync(actual, png);
      pairs.push(`fixture-01-${renderer}=${join(out, "fixture-01.native.png")},${actual}`);
    }
  } finally {
    await chrome.close();
    server.close();
  }

  const results = join(out, "results.json");
  const compare = spawnSync(
    native,
    ["compare", results, out, String(TOLERANCE.channel), String(TOLERANCE.fraction), ...pairs],
    { encoding: "utf8" },
  );
  const report = existsSync(results) ? JSON.parse(readFileSync(results, "utf8")) : [];
  assert.equal(report.length, 2, compare.stderr);
  for (const row of report) {
    assert.ok(
      row.passed,
      `${row.name}: ${row.differing_pixels} pixels differ (max ${row.max_channel_delta}); ${row.diff}`,
    );
  }
  assert.equal(compare.status, 0, compare.stderr);
});
