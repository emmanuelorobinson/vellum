import { describe, expect, it } from "vitest";
import { PIXEL_GRID_ZOOM, pixelGridLines } from "./pixel-grid";

describe("pixel grid", () => {
  it("is drawn only from 800% zoom, one line per model pixel, on whole screen pixels", () => {
    expect(PIXEL_GRID_ZOOM).toBe(8);
    expect(pixelGridLines({ x: 0, y: 0, zoom: 4 }, { width: 100, height: 100 })).toBeNull();
    const lines = pixelGridLines({ x: 3, y: -5, zoom: 8 }, { width: 40, height: 20 });
    // Screen x of world column k is 3 + 8k; visible columns from 0 to 40.
    expect(lines?.x).toEqual([3, 11, 19, 27, 35]);
    expect(lines?.y).toEqual([3, 11, 19]);
  });
  it("never asks for an unbounded number of lines", () => {
    const lines = pixelGridLines({ x: 0, y: 0, zoom: 8 }, { width: 1e7, height: 8 });
    expect(lines?.x.length).toBeLessThanOrEqual(4096);
  });
});
