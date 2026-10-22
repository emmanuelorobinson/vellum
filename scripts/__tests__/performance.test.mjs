import assert from "node:assert/strict";
import { test } from "node:test";
import { assess, summary } from "../performance-budget.mjs";

const measurements = () => ({
  machine: "TEST DATA",
  lane: "native",
  adapters: [
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
  metrics: Object.fromEntries(
    [
      "open2mb",
      "panZoom",
      "drag",
      "batch200",
      "idleRss",
      "codec-json-dir.open2mb",
      ...["open2mb", "panZoom", "drag", "batch200"].map((m) => `basic-native.${m}`),
    ].map((name) => [name, summary(Array(20).fill(1))]),
  ),
});
test("budgets and >10% regression are independent gates", () => {
  const base = measurements(),
    next = measurements();
  next.metrics.drag = summary(Array(20).fill(1.11));
  assert.match(assess(next, base).join("\n"), /drag.*regression/);
  next.metrics.drag = summary(Array(20).fill(16));
  assert.match(assess(next).join("\n"), /drag.*budget/);
  assert.deepEqual(assess(base, base), []);
});
test("missing, empty, malformed and nonfinite results fail closed", () => {
  for (const values of [[], [NaN], [Infinity], [-1]]) assert.throws(() => summary(values));
  for (const mutate of [
    (r) => {
      delete r.metrics.open2mb;
    },
    (r) => {
      r.adapters = [];
    },
    (r) => {
      r.metrics.drag.p95 = NaN;
    },
    (r) => {
      r.metrics.drag.samples = 0;
    },
  ]) {
    const r = measurements();
    mutate(r);
    assert.ok(assess(r).length);
  }
  assert.ok(assess(measurements(), { ...measurements(), machine: "other" }).length);
});
test("nearest rank p95, immutable input, exact 10% boundary", () => {
  const values = Array.from({ length: 20 }, (_, i) => 20 - i);
  assert.deepEqual(summary(values), { samples: 20, p50: 10, p95: 19, max: 20 });
  assert.equal(values[0], 20);
  const base = measurements(),
    next = measurements();
  next.metrics.drag = summary(Array(20).fill(1.1));
  assert.deepEqual(assess(next, base), []);
});
test("browser measures every shipped renderer; size and idle RSS have hard caps", () => {
  const native = measurements();
  native.metrics.idleRss = summary(Array(20).fill(129 * 1024 * 1024));
  assert.match(assess(native).join("\n"), /idleRss.*budget/);
  const r = {
    machine: "TEST DATA",
    lane: "browser",
    adapters: ["skia-webgl2", "skia-wasm-cpu", "canvas2d"],
    metrics: { wasmGzip: summary([3 * 1024 * 1024]) },
  };
  assert.match(assess(r).join("\n"), /wasmGzip.*budget/);
  assert.match(assess(r).join("\n"), /canvas2d.panZoom.*missing/);
});
test("a fixed candidate is allowed when the baseline exceeded an absolute budget", () => {
  const base = measurements();
  base.metrics.drag = summary(Array(20).fill(20));
  assert.deepEqual(assess(measurements(), base), []);
});
