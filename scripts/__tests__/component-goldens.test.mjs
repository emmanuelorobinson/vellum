import assert from "node:assert/strict";
import { test } from "node:test";
import { compareComponentPixels, componentGoldenCases } from "../lib/component-goldens.mjs";

const pixels = () => ({ width: 10, height: 10, data: new Uint8Array(400).fill(255) });
test("component pixel gate catches wrong size and corrupt renders", () => {
  assert.equal(compareComponentPixels(pixels(), pixels()).pass, true);
  assert.equal(compareComponentPixels(pixels(), { ...pixels(), width: 9 }).pass, false);
  const actual = pixels();
  actual.data.fill(0, 0, 8);
  assert.equal(compareComponentPixels(pixels(), actual).pass, false);
});
test("pending component goldens are counted separately from passes", async () => {
  let loads = 0;
  const result = await componentGoldenCases(
    [{ id: "pending" }, { id: "ready" }],
    async (c) => (c.id === "pending" ? { status: "pending" } : { status: "ready", ...pixels() }),
    async () => {
      loads++;
      return pixels();
    },
  );
  assert.deepEqual(result, { passed: ["ready"], pending: ["pending"], failed: [] });
  assert.equal(loads, 1);
});
test("a missing expected PNG or missing render state fails a built component", async () => {
  const result = await componentGoldenCases(
    [{ id: "missing" }, { id: "invalid" }],
    async (c) => ({ status: c.id === "missing" ? "ready" : "invalid", ...pixels() }),
    async () => {
      throw Error("Snapshot missing");
    },
  );
  assert.equal(result.failed.length, 2);
  assert.equal(result.passed.length, 0);
});
