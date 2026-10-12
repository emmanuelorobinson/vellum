// The text-fixture pipeline (guide Prompt 5), shared by measure-text.mjs and the gpu-tagged text test:
// render the 30 fixtures natively (Parley glyphs, and SkParagraph for comparison), capture them from
// Renderer A and Renderer B (GPU, plus B's CPU raster inside WASM), and pixel-diff every pair with
// vellum-testkit through `spike-skia-native compare`.

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { dataUrlToBuffer, launchChrome, openSpike } from "./chrome.mjs";

const here = dirname(fileURLToPath(import.meta.url));
export const spike = resolve(here, "../..");
export const native = join(spike, "target/release/spike-skia-native");
export const fontsDir = join(spike, "fonts");
export const TEXT_FIXTURE_COUNT = 30;

const id = (n) => String(n).padStart(2, "0");

/** Pairs compared, as [kind, expected suffix, actual suffix]. */
export const KINDS = [
  ["native-vs-a", "native", "a"],
  ["native-vs-b", "native", "b"],
  ["a-vs-b", "a", "b"],
  ["native-vs-b-raster", "native", "b-raster"],
  // Not a renderer path: the same text laid out by SkParagraph instead of Parley.
  ["parley-vs-skparagraph", "native", "paragraph"],
];

/** Renders and captures every text fixture into `dir`; returns text-layout.json's rows. */
export async function captureTextFixtures(url, dir, { renderers = ["a", "b"] } = {}) {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const run = spawnSync(native, ["text-fixtures", fontsDir, dir], { encoding: "utf8" });
  if (run.status !== 0) throw new Error(`native text render failed: ${run.stderr}`);
  const chrome = await launchChrome();
  try {
    for (const r of renderers) {
      for (let n = 1; n <= TEXT_FIXTURE_COUNT; n++) {
        await openSpike(chrome.cdp, `${url}?renderer=${r}&text=${n}`);
        const png = await chrome.cdp.evaluate("window.__spike.then(s => s.captureDataUrl())");
        writeFileSync(join(dir, `text-${id(n)}.${r}.png`), dataUrlToBuffer(png));
        if (r === "b") {
          const raster = await chrome.cdp.evaluate(
            "window.__spike.then(s => s.captureDataUrl(true))",
          );
          writeFileSync(join(dir, `text-${id(n)}.b-raster.png`), dataUrlToBuffer(raster));
        }
      }
    }
  } finally {
    await chrome.close();
  }
  return JSON.parse(readFileSync(join(dir, "text-layout.json"), "utf8"));
}

/**
 * Pixel-diffs `kinds` for every fixture with the given tolerance. Returns the rows from
 * `spike-skia-native compare` (one per fixture and kind) and its exit status.
 */
export function compareTextFixtures(dir, kinds, { channel = 2, fraction = 0 } = {}) {
  const pairs = [];
  for (let n = 1; n <= TEXT_FIXTURE_COUNT; n++) {
    for (const [kind, expected, actual] of kinds) {
      const f = (k) => join(dir, `text-${id(n)}.${k}.png`);
      pairs.push(`${id(n)}-${kind}=${f(expected)},${f(actual)}`);
    }
  }
  const json = join(dir, "compare.json");
  const run = spawnSync(
    native,
    ["compare", json, join(dir, "diffs"), String(channel), String(fraction), ...pairs],
    { encoding: "utf8" },
  );
  if (run.status === 2) throw new Error(`compare failed: ${run.stderr}`);
  const rows = JSON.parse(readFileSync(json, "utf8")).map((row) => ({
    ...row,
    diff: row.diff ? row.diff.slice(spike.length + 1) : "",
  }));
  rmSync(json);
  return { rows, passed: run.status === 0 };
}

export function summarize(rows, kind) {
  const subset = rows.filter((row) => row.name.endsWith(kind));
  const p95 = subset.map((row) => row.p95_pixel_delta).sort((a, b) => a - b);
  return {
    fixtures: subset.length,
    identical: subset.filter((row) => row.max_channel_delta === 0).length,
    withinChannel2: subset.filter((row) => row.differing_pixels === 0).length,
    worstMaxDelta: Math.max(...subset.map((row) => row.max_channel_delta)),
    medianMaxDelta: subset.map((row) => row.max_channel_delta).sort((a, b) => a - b)[
      Math.floor(subset.length / 2)
    ],
    worstP95: p95.at(-1),
    worstDifferingFraction: Math.max(...subset.map((row) => row.differing_fraction)),
    meanDifferingFraction: subset.reduce((s, row) => s + row.differing_fraction, 0) / subset.length,
  };
}
