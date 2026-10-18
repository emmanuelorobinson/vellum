import { describe, expect, it } from "vitest";
import { fitViewport, MAX_ZOOM, MIN_ZOOM, nextZoomStep, zoomAbout } from "./viewport";

describe("viewport maths", () => {
  it("zooms about a screen point and keeps the world point under it still", () => {
    const v = zoomAbout({ x: 0, y: 0, zoom: 1 }, { x: 200, y: 100 }, 2);
    expect(v).toEqual({ x: -200, y: -100, zoom: 2 });
  });
  it("clamps zoom to the supported range", () => {
    expect(zoomAbout({ x: 0, y: 0, zoom: 1 }, { x: 0, y: 0 }, 1e9).zoom).toBe(MAX_ZOOM);
    expect(zoomAbout({ x: 0, y: 0, zoom: 1 }, { x: 0, y: 0 }, 0).zoom).toBe(MIN_ZOOM);
  });
  it("steps through the usual zoom stops", () => {
    expect(nextZoomStep(1, "in")).toBe(2);
    expect(nextZoomStep(1, "out")).toBe(0.5);
    expect(nextZoomStep(0.72, "in")).toBe(1);
    expect(nextZoomStep(0.72, "out")).toBe(0.5);
    expect(nextZoomStep(MAX_ZOOM, "in")).toBe(MAX_ZOOM);
    expect(nextZoomStep(MIN_ZOOM, "out")).toBe(MIN_ZOOM);
  });
  it("fits a box into the free area, centred, with a margin", () => {
    const v = fitViewport(
      { x: 0, y: 0, width: 400, height: 200 },
      { width: 1000, height: 600 },
      { left: 100, right: 100 },
      { margin: 50, maxZoom: 1 },
    );
    // free area 800 x 600, minus a 50 margin each side: 700 x 500; 400 fits at 1 (capped), centred at x 500.
    expect(v.zoom).toBe(1);
    expect(v.x).toBeCloseTo(100 + 400 - 200);
    expect(v.y).toBeCloseTo(300 - 100);
  });
  it("zooms out to fit something large", () => {
    const v = fitViewport(
      { x: 0, y: 0, width: 4000, height: 1000 },
      { width: 1000, height: 600 },
      { left: 0, right: 0 },
      { margin: 0, maxZoom: 1 },
    );
    expect(v.zoom).toBeCloseTo(0.25);
  });
  it("falls back to 100% at the origin for an empty box", () => {
    expect(
      fitViewport(
        { x: 5, y: 5, width: 0, height: 0 },
        { width: 100, height: 100 },
        { left: 0, right: 0 },
      ),
    ).toMatchObject({ zoom: 1 });
  });
});
