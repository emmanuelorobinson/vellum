import { describe, expect, it } from "vitest";
import { checkViewport } from "./index";

describe("Renderer input contract", () => {
  it("converts logical size and DPR to bounded device dimensions", () => {
    expect(checkViewport({ width: 20, height: 10, dpr: 2 })).toEqual([40, 20]);
    for (const width of [0, -1, Infinity, NaN, 20000]) {
      expect(() => checkViewport({ width, height: 10, dpr: 1 })).toThrow();
    }
  });
});
