// ADR-004 QuickJS numbers (guide Prompt 5): the wasm32 size rquickjs adds to vellum-wasm, and the
// 500-op script's run time in wasm32 (Node) and natively. Needs `bash scripts/build-wasm.sh`.
//
//   node scripts/measure-script.mjs [--runs N]      writes results/script.json

import { spawnSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

import { engine, instantiate, scriptWasm, vellumWasm } from "./lib.mjs";

const i = process.argv.indexOf("--runs");
const RUNS = i > 0 ? Number(process.argv[i + 1]) : 200;
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const p95 = (xs) => [...xs].sort((a, b) => a - b)[Math.ceil(xs.length * 0.95) - 1];
const size = (f) => ({
  raw: statSync(f).size,
  gzip: gzipSync(readFileSync(f), { level: 9 }).length,
});

function time(fn) {
  const xs = [];
  for (let k = 0; k < 10; k++) fn(); // warm-up
  for (let k = 0; k < RUNS; k++) {
    const t = performance.now();
    const ops = fn();
    xs.push(performance.now() - t);
    if (ops !== 500) throw new Error(`script made ${ops} ops`);
  }
  return { medianMs: +median(xs).toFixed(3), p95Ms: +p95(xs).toFixed(3), runs: RUNS };
}

const t0 = performance.now();
const { imports, exports } = instantiate();
const instantiateMs = performance.now() - t0;
const wasm = {
  cold: time(() => exports.script_run_500_cold()),
  warm: time(() => exports.script_run_500_warm()),
};
const native = spawnSync(
  "cargo",
  ["run", "-q", "--release", "-p", "spike-script", "--bin", "script-bench", "--", String(RUNS)],
  { cwd: engine, encoding: "utf8" },
);
if (native.status !== 0) throw new Error(native.stderr);
const base = size(vellumWasm);
const withScript = size(scriptWasm);
const out = {
  measuredAt: new Date().toISOString(),
  machine: {
    cpu: os.cpus()[0]?.model,
    platform: `${os.platform()} ${os.release()}`,
    node: process.version,
  },
  method:
    "wasm32-unknown-unknown, Cargo's default release profile (as the product workspace); Node's V8. cold = new runtime + context + script; warm = script on a kept engine",
  size: {
    vellumWasm: base,
    vellumWasmWithQuickjs: withScript,
    addedRaw: withScript.raw - base.raw,
    addedGzip: withScript.gzip - base.gzip,
  },
  imports,
  instantiateMs: +instantiateMs.toFixed(2),
  wasm,
  native: JSON.parse(native.stdout),
};
writeFileSync(join(engine, "results/script.json"), `${JSON.stringify(out, null, 2)}\n`);
console.error(JSON.stringify({ size: out.size, wasm, native: out.native }, null, 1));
