// ADR-004 Loro numbers in WASM (guide Prompt 5) on wasm32-unknown-emscripten, the target of the editor
// module (ADR-001). Needs `bash scripts/build-wasm.sh`. Node times each exported step; medians of N runs.
//
//   node scripts/measure-loro-wasm.mjs [--runs N]      writes results/loro-wasm.json

import { readFileSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

import { engine } from "./lib.mjs";

const out_dir = join(engine, "target/wasm32-unknown-emscripten/release");
const wasm = join(out_dir, "loro_wasm.wasm");
const { default: createLoroSpike } = await import(join(out_dir, "loro_wasm.js"));
const i = process.argv.indexOf("--runs");
const RUNS = i > 0 ? Number(process.argv[i + 1]) : 7;
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

function time(fn) {
  const t = performance.now();
  const value = fn();
  return [value, performance.now() - t];
}

const rows = [];
for (const n of [1000, 5000, 10000, 25000, 50000]) {
  const t = {};
  const push = (k, v) => {
    t[k] = [...(t[k] ?? []), v];
  };
  let bytes = {};
  for (let run = 0; run < RUNS; run++) {
    // A fresh instance per run, so memory growth from earlier runs does not carry over.
    const M = await createLoroSpike();
    const e = Object.fromEntries(
      Object.keys(M)
        .filter((k) => k.startsWith("_lb_"))
        .map((k) => [k.slice(1), M[k]]),
    );
    const nodes = e.lb_generate(n);
    const [built, build] = time(() => e.lb_build());
    if (built !== nodes) throw new Error(`tree has ${built} nodes, scene ${nodes}`);
    const [snapshot, save] = time(() => e.lb_save());
    const [, load] = time(() => e.lb_load());
    const [, read] = time(() => e.lb_full_read());
    e.lb_prepare_batch(BigInt(run));
    const [, batch] = time(() => e.lb_apply_batch());
    const [update, exportUpdate] = time(() => e.lb_export_update());
    const [, remote] = time(() => e.lb_remote_import());
    if (e.lb_converged() !== 1) throw new Error("remote did not converge");
    for (const [k, v] of Object.entries({ build, save, load, read, batch, exportUpdate, remote }))
      push(k, v);
    bytes = { snapshot, update, memory: M.HEAPU8.length };
  }
  const m = Object.fromEntries(Object.entries(t).map(([k, xs]) => [k, +median(xs).toFixed(2)]));
  rows.push({
    nodes: n,
    runs: RUNS,
    buildMs: m.build,
    saveMs: m.save,
    snapshotBytes: bytes.snapshot,
    loadMs: m.load,
    firstFullReadMs: m.read,
    batchApplyMs: m.batch,
    batchExportUpdateMs: m.exportUpdate,
    batchUpdateBytes: bytes.update,
    batchRemoteImportMs: m.remote,
    wasmMemoryBytesAfterRun: bytes.memory,
  });
  console.error(JSON.stringify(rows.at(-1)));
}
const out = {
  measuredAt: new Date().toISOString(),
  machine: {
    cpu: os.cpus()[0]?.model,
    platform: `${os.platform()} ${os.release()}`,
    node: process.version,
  },
  method:
    "wasm32-unknown-emscripten (emsdk 4.0.6, -O3 as rustc links it), Cargo's default release profile, Node's V8; the same steps as loro-bench; first remote import after load (the remote's state is decoded first)",
  module: {
    file: "loro_wasm.wasm (Loro plus the spike scene generator)",
    raw: statSync(wasm).size,
    gzip: gzipSync(readFileSync(wasm), { level: 9 }).length,
  },
  rows,
};
writeFileSync(join(engine, "results/loro-wasm.json"), `${JSON.stringify(out, null, 2)}\n`);
