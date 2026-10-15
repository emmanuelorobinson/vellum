import assert from "node:assert/strict";
import { test } from "node:test";
import { comparePixels } from "../renderer-golden-compare.mjs";

test("golden comparator detects corruption, respects tolerance, and emits a visible diff", () => {
  const a = new Uint8Array([255, 0, 0, 255, 0, 0, 0, 0]),
    b = a.slice();
  b[0] = 250;
  const t = { channel: 2, fraction: 0, p95: 2 };
  assert.equal(comparePixels(a, a, t).pass, true);
  const changed = comparePixels(a, b, t);
  assert.equal(changed.pass, false);
  assert.equal(changed.diff[0], 5);
  assert.equal(changed.diff[3], 255);
  assert.equal(comparePixels(a, b, { channel: 5, fraction: 0, p95: 5 }).pass, true);
  assert.equal(comparePixels(a, b.slice(1), t).pass, false);
});
test("comparison measures composited pixels: invisible RGB is irrelevant and alpha changes fail", () => {
  const tolerance = { channel: 0, fraction: 0, p95: 0 };
  assert.equal(
    comparePixels(new Uint8Array([255, 0, 0, 0]), new Uint8Array([0, 255, 255, 0]), tolerance).pass,
    true,
  );
  assert.equal(
    comparePixels(new Uint8Array([0, 0, 0, 128]), new Uint8Array([0, 0, 0, 129]), tolerance).pass,
    false,
  );
});
