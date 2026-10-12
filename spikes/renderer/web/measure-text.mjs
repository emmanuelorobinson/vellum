// ADR-001 text results (guide Prompt 5). Needs the spike build (scripts/build.sh), the fixture fonts
// (node scripts/fetch-fonts.mjs) and a local Chrome.
//
//   node measure-text.mjs
//
// Writes spikes/renderer/results/text.json: per-pair and per-category pixel differences for the 30 text
// fixtures (native CPU raster, A and B on the GPU, B's CPU raster inside WASM, and SkParagraph vs Parley
// layout), the line-count comparison, and the WASM size Parley adds. Captures and diff images go to
// results/text-fixtures/ (gitignored).

import { readFileSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";
import {
  captureTextFixtures,
  compareTextFixtures,
  KINDS,
  spike,
  summarize,
  TEXT_FIXTURE_COUNT,
} from "./lib/text.mjs";
import { startServer } from "./server.mjs";

const here = join(spike, "web");
const results = join(spike, "results");
const dir = join(results, "text-fixtures");
const log = (...m) => console.error("[measure-text]", ...m);

function size(files) {
  const list = files.map((f) => {
    const bytes = readFileSync(join(here, f));
    return {
      file: f,
      raw: statSync(join(here, f)).size,
      gzip: gzipSync(bytes, { level: 9 }).length,
    };
  });
  return {
    files: list,
    raw: list.reduce((s, f) => s + f.raw, 0),
    gzip: list.reduce((s, f) => s + f.gzip, 0),
  };
}

const { server, url } = await startServer();
try {
  log("capturing");
  const layout = await captureTextFixtures(url, dir);
  log("comparing");
  const { rows } = compareTextFixtures(dir, KINDS);
  const categories = {};
  for (let n = 1; n <= TEXT_FIXTURE_COUNT; n++) {
    const category = layout[n - 1].name.split("-")[0];
    categories[category] = [...(categories[category] ?? []), n];
  }
  const byCategory = {};
  for (const [category, fixtures] of Object.entries(categories)) {
    const ids = new Set(fixtures.map((n) => String(n).padStart(2, "0")));
    const subset = rows.filter((row) => ids.has(row.name.slice(0, 2)));
    byCategory[category] = Object.fromEntries(
      KINDS.map(([kind]) => [kind, summarize(subset, kind)]),
    );
  }
  const prompt4 = JSON.parse(readFileSync(join(results, "results.json"), "utf8")).wasmSize;
  const out = {
    measuredAt: new Date().toISOString(),
    machine: {
      cpu: os.cpus()[0]?.model,
      platform: `${os.platform()} ${os.release()}`,
    },
    method: {
      fixtures: "30 text fixtures, 256 x 256, laid out by Parley 0.12.0 inside each module",
      font: "anti-aliased, subpixel positioning, no hinting, linear metrics; FreeType everywhere",
      tolerance: "a pixel differs when any channel moves by more than 2",
    },
    summary: Object.fromEntries(KINDS.map(([kind]) => [kind, summarize(rows, kind)])),
    byCategory,
    lineCounts: layout,
    lineCountMismatches: layout.filter((r) => r.parleyLines !== r.paragraphLines).length,
    wasmSize: {
      "a: scene module with Parley": size(["dist/spike_scene_wasm.wasm"]),
      "b: skia_wasm with Parley": size(["dist/skia_wasm.wasm", "dist/skia_wasm.js"]),
      "b: skia_wasm with Parley and rquickjs (size only)": size([
        "dist/quickjs/skia_wasm.wasm",
        "dist/quickjs/skia_wasm.js",
      ]),
      "prompt 4 b (no Parley)": prompt4?.b,
      "prompt 4 b with textlayout (no Parley)": prompt4?.["b with textlayout (size only)"],
    },
    rows,
  };
  writeFileSync(join(results, "text.json"), `${JSON.stringify(out, null, 2)}\n`);
  log("summary", JSON.stringify(out.summary, null, 1));
  log(`wrote ${join(results, "text.json")}`);
} finally {
  server.close();
}
