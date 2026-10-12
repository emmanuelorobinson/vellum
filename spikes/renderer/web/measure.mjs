// ADR-001 measurements (guide Prompt 4). Needs the spike build (scripts/build.sh) and a local Chrome.
//
//   node measure.mjs [--runs N] [--seconds S] [--software]
//   node measure.mjs --sizes-only      refresh only the WASM sizes in results.json
//   node measure.mjs --frames-only     cold load and frame numbers only, into results/frames-<host>.json;
//                                      needs web/dist and Chrome but not the native toolchain, so the
//                                      build can be copied to another machine (an M1, a Windows laptop)
//
// Writes spikes/renderer/results/: results.json (every number in ADR-001), the fixture captures and
// the diff images for fixtures that differ. Measures, per renderer:
//   - frames per second while panning and zooming over 1,000 / 5,000 / 10,000 nodes (vsync off)
//   - drag input -> frame latency (pointer event timestamp to GPU completion of the frame using it)
//   - WASM size (raw and gzip -9) and cold load (navigation start to first frame on the GPU, fresh profile)
//   - native vs WASM pixel difference on the 20 non-text fixtures (GPU paths, and B's CPU raster in WASM)
// --software adds the same frame numbers with SwiftShader (software WebGL), a lower bound for machines
// whose GPU Chrome blocks.

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

import { dataUrlToBuffer, launchChrome, openSpike } from "./lib/chrome.mjs";
import { startServer } from "./server.mjs";
import { percentile } from "./src/stats.js";

const here = dirname(fileURLToPath(import.meta.url));
const spike = resolve(here, "..");
const results = join(spike, "results");
const native = join(spike, "target/release/spike-skia-native");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? Number(args[i + 1]) : fallback;
};
const RUNS = option("--runs", 3);
const SECONDS = option("--seconds", 5);
const SIZES = [1000, 5000, 10000];
const RENDERERS = ["a", "b"];
const median = (xs) => percentile(xs, 0.5);
const log = (...m) => console.error("[measure]", ...m);

function sizes() {
  const ck = join(here, "node_modules/canvaskit-wasm/bin");
  const files = {
    a: [
      join(here, "dist/spike_scene_wasm.wasm"),
      join(ck, "canvaskit.wasm"),
      join(ck, "canvaskit.js"),
      join(here, "src/renderer-a.js"),
    ],
    b: [
      join(here, "dist/skia_wasm.wasm"),
      join(here, "dist/skia_wasm.js"),
      join(here, "src/renderer-b.js"),
    ],
    "b with textlayout (size only)": [
      join(here, "dist/textlayout/skia_wasm.wasm"),
      join(here, "dist/textlayout/skia_wasm.js"),
      join(here, "src/renderer-b.js"),
    ],
    "canvaskit-full (reference)": [join(ck, "full/canvaskit.wasm"), join(ck, "full/canvaskit.js")],
  };
  const out = {};
  for (const [key, list] of Object.entries(files)) {
    out[key] = {
      files: list.map((f) => {
        const bytes = readFileSync(f);
        return {
          file: f.slice(spike.length + 1),
          raw: statSync(f).size,
          gzip: gzipSync(bytes, { level: 9 }).length,
        };
      }),
    };
    out[key].raw = out[key].files.reduce((s, f) => s + f.raw, 0);
    out[key].gzip = out[key].files.reduce((s, f) => s + f.gzip, 0);
  }
  return out;
}

async function coldLoad(url) {
  const out = {};
  for (const r of RENDERERS) out[r] = [];
  for (let run = 0; run < Math.max(RUNS, 5); run++) {
    // Alternate the order so neither renderer always goes first after a fresh launch.
    for (const r of run % 2 === 0 ? RENDERERS : [...RENDERERS].reverse()) {
      const chrome = await launchChrome();
      try {
        await chrome.cdp.send("Network.enable");
        await chrome.cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
        const info = await openSpike(chrome.cdp, `${url}?renderer=${r}&nodes=1000`);
        out[r].push(info.firstFrameMs);
      } finally {
        await chrome.close();
      }
    }
  }
  return Object.fromEntries(
    Object.entries(out).map(([r, xs]) => [
      r,
      { runs: xs.map((x) => Math.round(x)), median: Math.round(median(xs)) },
    ]),
  );
}

async function drag(cdp, width, height, moves = 90) {
  const y = height / 2;
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: width / 2,
    y,
    button: "left",
    clickCount: 1,
  });
  for (let i = 0; i < moves; i++) {
    const x = width / 2 + Math.sin(i / 10) * width * 0.3;
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x,
      y,
      button: "left",
      buttons: 1,
    });
    await new Promise((ok) => setTimeout(ok, 12));
  }
  await cdp.send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    x: width / 2,
    y,
    button: "left",
    clickCount: 1,
  });
}

async function frames(url, { software = false } = {}) {
  const chrome = await launchChrome({
    extraArgs: software ? ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] : [],
  });
  const out = {};
  let glRenderer = "";
  let canvas = "";
  try {
    for (const n of SIZES) {
      for (const r of RENDERERS) {
        const runs = [];
        const latencies = [];
        for (let run = 0; run < RUNS; run++) {
          const info = await openSpike(chrome.cdp, `${url}?renderer=${r}&nodes=${n}`);
          glRenderer = info.glRenderer;
          canvas = `${info.width} x ${info.height} device px`;
          runs.push(await chrome.cdp.evaluate(`window.__spike.then(s => s.bench(${SECONDS}))`));
          if (!software) {
            await chrome.cdp.evaluate("window.__spike.then(s => s.trackLatency(true))");
            const { width, height } = await chrome.cdp.evaluate(
              "(() => { const r = document.getElementById('canvas').getBoundingClientRect(); return { width: r.width, height: r.height + r.top }; })()",
            );
            await drag(chrome.cdp, width, height);
            latencies.push(await chrome.cdp.evaluate("window.__spike.then(s => s.latency())"));
          }
        }
        const pick = (key) => median(runs.map((x) => x[key]));
        out[`${r}-${n}`] = {
          renderer: r,
          nodes: n,
          fps: +pick("fps").toFixed(1),
          frameMsP50: +pick("frameMsP50").toFixed(2),
          frameMsP95: +pick("frameMsP95").toFixed(2),
          drawCallMsP50: +pick("drawMsP50").toFixed(2),
          drawCallMsP95: +pick("drawMsP95").toFixed(2),
          runs: runs.map((x) => +x.fps.toFixed(1)),
          ...(latencies.length && {
            latencyMsP50: +median(latencies.map((l) => l.p50)).toFixed(1),
            latencyMsP95: +median(latencies.map((l) => l.p95)).toFixed(1),
            latencySamples: latencies.reduce((s, l) => s + l.samples, 0),
          }),
        };
        log(software ? "software" : "gpu", r, n, JSON.stringify(out[`${r}-${n}`]));
      }
    }
  } finally {
    await chrome.close();
  }
  return { glRenderer, canvas, results: out };
}

async function fixtures(url) {
  const dir = join(results, "fixtures");
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const nativeRun = spawnSync(native, ["fixtures", dir], { encoding: "utf8" });
  if (nativeRun.status !== 0) throw new Error(`native render failed: ${nativeRun.stderr}`);
  const chrome = await launchChrome();
  try {
    for (const r of RENDERERS) {
      for (let n = 1; n <= 20; n++) {
        await openSpike(chrome.cdp, `${url}?renderer=${r}&fixture=${n}`);
        const png = dataUrlToBuffer(
          await chrome.cdp.evaluate("window.__spike.then(s => s.captureDataUrl())"),
        );
        writeFileSync(join(dir, `fixture-${String(n).padStart(2, "0")}.${r}.png`), png);
        if (r === "b") {
          const raster = await chrome.cdp.evaluate(
            "window.__spike.then(s => s.captureDataUrl(true))",
          );
          writeFileSync(
            join(dir, `fixture-${String(n).padStart(2, "0")}.b-raster.png`),
            dataUrlToBuffer(raster),
          );
        }
      }
    }
  } finally {
    await chrome.close();
  }
  const pairs = [];
  for (let n = 1; n <= 20; n++) {
    const f = (k) => join(dir, `fixture-${String(n).padStart(2, "0")}.${k}.png`);
    const id = String(n).padStart(2, "0");
    pairs.push(
      `${id}-native-vs-a=${f("native")},${f("a")}`,
      `${id}-native-vs-b=${f("native")},${f("b")}`,
      `${id}-a-vs-b=${f("a")},${f("b")}`,
      `${id}-native-vs-b-raster=${f("native")},${f("b-raster")}`,
    );
  }
  // Strict numbers: a pixel counts as different when any channel moves by more than 2.
  const json = join(dir, "compare.json");
  spawnSync(native, ["compare", json, join(dir, "diffs"), "2", "0", ...pairs], {
    encoding: "utf8",
  });
  const rows = JSON.parse(readFileSync(json, "utf8")).map((row) => ({
    ...row,
    diff: row.diff ? row.diff.slice(spike.length + 1) : "",
  }));
  rmSync(json);
  const summary = {};
  for (const kind of ["native-vs-a", "native-vs-b", "a-vs-b", "native-vs-b-raster"]) {
    const subset = rows.filter((row) => row.name.endsWith(kind));
    summary[kind] = {
      identical: subset.filter((row) => row.max_channel_delta === 0).length,
      withinChannel2: subset.filter((row) => row.passed).length,
      worstMaxDelta: Math.max(...subset.map((row) => row.max_channel_delta)),
      worstP95: Math.max(...subset.map((row) => row.p95_pixel_delta)),
      worstDifferingFraction: Math.max(...subset.map((row) => row.differing_fraction)),
      meanDifferingFraction:
        subset.reduce((s, row) => s + row.differing_fraction, 0) / subset.length,
    };
  }
  return { tolerance: { channel: 2 }, summary, rows };
}

const { server, url } = await startServer();
try {
  mkdirSync(results, { recursive: true });
  const out = {
    measuredAt: new Date().toISOString(),
    machine: {
      cpu: os.cpus()[0]?.model,
      platform: `${os.platform()} ${os.release()}`,
      memoryGb: Math.round(os.totalmem() / 2 ** 30),
    },
    method: {
      runs: RUNS,
      benchSeconds: SECONDS,
      chrome: "headless=new, --disable-gpu-vsync --disable-frame-rate-limit",
      window: "1280 x 860, DPR 1",
    },
  };
  if (args.includes("--sizes-only")) {
    const file = join(results, "results.json");
    const previous = JSON.parse(readFileSync(file, "utf8"));
    writeFileSync(file, `${JSON.stringify({ ...previous, wasmSize: sizes() }, null, 2)}\n`);
    log(`updated sizes in ${file}`);
    process.exit(0);
  }
  if (args.includes("--frames-only")) {
    out.coldLoadMs = await coldLoad(url);
    out.gpu = await frames(url);
    const file = join(results, `frames-${os.hostname().replace(/[^\w.-]/g, "_")}.json`);
    writeFileSync(file, `${JSON.stringify(out, null, 2)}\n`);
    log(`wrote ${file}`);
    process.exit(0);
  }
  log("sizes");
  out.wasmSize = sizes();
  log("fixtures");
  out.pixels = await fixtures(url);
  log("cold load");
  out.coldLoadMs = await coldLoad(url);
  log("frames (gpu)");
  out.gpu = await frames(url);
  if (args.includes("--software")) {
    log("frames (software)");
    out.software = await frames(url, { software: true });
  }
  writeFileSync(join(results, "results.json"), `${JSON.stringify(out, null, 2)}\n`);
  log(`wrote ${join(results, "results.json")}`);
} finally {
  server.close();
}
