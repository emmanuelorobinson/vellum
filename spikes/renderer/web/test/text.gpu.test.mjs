// Tagged `gpu` and `fonts` (guide §11 rule 19). Needs: Chrome with a working GPU, the spike build
// (`bash spikes/renderer/scripts/build.sh`: emsdk 4.0.6, ninja) and the fixture fonts
// (`node spikes/renderer/scripts/fetch-fonts.mjs`, network once). Runs only when VELLUM_LANES contains
// both "gpu" and "fonts"; otherwise it is reported as skipped, never as passed.
//
// The 30 text fixtures (Prompt 5), laid out by Parley inside each module, must render:
// - identically in Renderer A (CanvasKit) and Renderer B (skia-safe in WASM): same Ganesh, same glyphs;
// - within ±2 per channel on at most 0.1% of pixels against the native CPU raster. Measured on the
//   M5 Pro: at most 1 pixel of 65,536 differs, by at most 3 (results/text.json).

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { chromePath } from "../lib/chrome.mjs";
import { captureTextFixtures, compareTextFixtures, fontsDir, native, spike } from "../lib/text.mjs";
import { startServer } from "../server.mjs";

const lanes = (process.env.VELLUM_LANES ?? "").split(",").map((s) => s.trim());
const out = join(spike, "artifacts/text-gpu-test");

function skipReason() {
  if (!lanes.includes("gpu") || !lanes.includes("fonts"))
    return "gpu and fonts lanes not enabled (set VELLUM_LANES=gpu,fonts)";
  if (!chromePath()) return "Chrome not found (set VELLUM_CHROME)";
  return null;
}

test("text fixtures render the same in both browser paths and natively", {
  skip: skipReason() ?? false,
}, async () => {
  for (const file of [
    native,
    join(spike, "web/dist/spike_scene_wasm.wasm"),
    join(spike, "web/dist/skia_wasm.wasm"),
  ]) {
    assert.ok(existsSync(file), `${file} is missing; run spikes/renderer/scripts/build.sh`);
  }
  assert.ok(
    existsSync(join(fontsDir, "NotoSans-Regular.ttf")),
    "fixture fonts are missing; run node spikes/renderer/scripts/fetch-fonts.mjs",
  );

  const { server, url } = await startServer();
  try {
    await captureTextFixtures(url, out);
  } finally {
    server.close();
  }

  const identical = compareTextFixtures(out, [["a-vs-b", "a", "b"]], { channel: 0, fraction: 0 });
  for (const row of identical.rows)
    assert.ok(row.passed, `${row.name}: ${row.differing_pixels} pixels differ; ${row.diff}`);

  const close = compareTextFixtures(
    out,
    [
      ["native-vs-a", "native", "a"],
      ["native-vs-b", "native", "b"],
    ],
    { channel: 2, fraction: 0.001 },
  );
  assert.equal(close.rows.length, 60);
  for (const row of close.rows)
    assert.ok(
      row.passed,
      `${row.name}: ${row.differing_pixels} pixels differ (max ${row.max_channel_delta}); ${row.diff}`,
    );
});
