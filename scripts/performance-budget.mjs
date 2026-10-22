// §12 absolute budgets, independently checked from the paired, same-machine 10% regression gate.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
export const budgets = Object.freeze({
  open2mb: 500,
  panZoom: 1000 / 60,
  drag: 16,
  batch200: 100,
  wasmGzip: 2.4 * 1024 * 1024,
  idleRss: 128 * 1024 * 1024,
});
export const adapters = Object.freeze({
  native: [
    "assets-fs",
    "codec-json",
    "codec-json-dir",
    "doc-loro",
    "layout-taffy",
    "text-parley",
    "index-rstar",
    "render-skia",
    "ipc-local",
  ],
  browser: ["skia-webgl2", "skia-wasm-cpu", "canvas2d"],
});
export function summary(values) {
  if (!Array.isArray(values) || !values.length || values.some((n) => !Number.isFinite(n) || n < 0))
    throw Error("Invalid performance samples");
  const sorted = [...values].sort((a, b) => a - b);
  return {
    samples: sorted.length,
    p50: sorted[Math.ceil(sorted.length * 0.5) - 1],
    p95: sorted[Math.ceil(sorted.length * 0.95) - 1],
    max: sorted.at(-1),
  };
}
export function assess(report, baseline, absolute = true) {
  const failures = [];
  if (
    !report ||
    !adapters[report.lane] ||
    typeof report.machine !== "string" ||
    !report.machine ||
    !Array.isArray(report.adapters) ||
    !report.metrics ||
    typeof report.metrics !== "object" ||
    Array.isArray(report.metrics)
  )
    return ["Invalid performance report"];
  for (const a of adapters[report.lane])
    if (!report.adapters?.includes(a)) failures.push(`${a}: adapter missing`);
  const required =
    report.lane === "native"
      ? [
          "open2mb",
          "panZoom",
          "drag",
          "batch200",
          "idleRss",
          "codec-json-dir.open2mb",
          ...["open2mb", "panZoom", "drag", "batch200"].map((m) => `basic-native.${m}`),
        ]
      : [
          "wasmGzip",
          ...adapters.browser.flatMap((a) =>
            ["panZoom", "drag", "batch200"].map((m) => `${a}.${m}`),
          ),
        ];
  if (baseline && (baseline.machine !== report.machine || baseline.lane !== report.lane))
    failures.push("Baseline must use the same machine and lane");
  if (baseline) for (const f of assess(baseline, undefined, false)) failures.push(`Baseline: ${f}`);
  for (const name of new Set([...required, ...Object.keys(report.metrics ?? {})])) {
    const v = report.metrics?.[name],
      key = name.split(".").at(-1),
      limit = budgets[key];
    if (
      !v ||
      !Number.isInteger(v.samples) ||
      v.samples < (key === "wasmGzip" ? 1 : 20) ||
      ![v.p50, v.p95, v.max].every((n) => Number.isFinite(n) && n >= 0) ||
      v.p50 > v.p95 ||
      v.p95 > v.max ||
      limit === undefined
    ) {
      failures.push(`${name}: missing or invalid measurement`);
      continue;
    }
    if (absolute && v.p95 >= limit) failures.push(`${name}: p95 ${v.p95} exceeds budget ${limit}`);
    if (baseline) {
      const b = baseline.metrics?.[name];
      if (!b) failures.push(`${name}: baseline missing`);
      else if (v.p95 > b.p95 * 1.1) failures.push(`${name}: >10% regression (${b.p95} → ${v.p95})`);
    }
  }
  return failures;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [file, base] = process.argv.slice(2);
  if (!file) throw Error("Usage: performance-budget.mjs REPORT.json [BASELINE.json]");
  const failures = assess(
    JSON.parse(readFileSync(file)),
    base ? JSON.parse(readFileSync(base)) : undefined,
  );
  for (const failure of failures) console.error(failure);
  if (failures.length) process.exitCode = 1;
}
