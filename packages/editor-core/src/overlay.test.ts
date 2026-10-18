import { describe, expect, it } from "vitest";
import { distanceLabels, HANDLE_SIZE, type OverlayInput, overlayShapes, toScreen } from "./overlay";

const empty = { handles: [], guides: [], measurements: [], marquee: null, pathAddress: null };
const view = { x: 100, y: 50, zoom: 2 };
const input = (over: Partial<OverlayInput> = {}): OverlayInput => ({
  overlay: empty,
  viewport: view,
  size: { width: 800, height: 600 },
  selection: [],
  previewBounds: [],
  frameLabels: [],
  hover: null,
  altMeasure: false,
  ...over,
});

describe("world to screen", () => {
  it("applies the viewport's translation and zoom", () => {
    expect(toScreen({ x: 10, y: 20 }, view)).toEqual({ x: 120, y: 90 });
  });
});

describe("selection and handles", () => {
  it("draws each selected layer's outline in the colour of what it is", () => {
    const shapes = overlayShapes(
      input({
        selection: [
          { box: { x: 0, y: 0, width: 10, height: 10 }, kind: "frame", component: false },
          { box: { x: 20, y: 0, width: 10, height: 10 }, kind: "frame", component: true },
          { box: { x: 40, y: 0, width: 10, height: 10 }, kind: "instance", component: false },
        ],
      }),
    );
    expect(
      shapes.filter((s) => s.kind === "outline").map((s) => s.kind === "outline" && s.role),
    ).toEqual(["selection", "component", "instance"]);
    const first = shapes.find((s) => s.kind === "outline");
    expect(first).toMatchObject({ box: { x: 100, y: 50, width: 20, height: 20 } });
  });
  it("places corner handles from the controller's world points and never scales them", () => {
    const shapes = overlayShapes(
      input({
        overlay: {
          ...empty,
          handles: [
            { point: { x: 0, y: 0 }, kind: "corner" },
            { point: { x: 10, y: 10 }, kind: "corner" },
          ],
        },
      }),
    );
    const handles = shapes.filter((s) => s.kind === "handle");
    expect(handles).toEqual([
      { kind: "handle", role: "corner", at: { x: 100, y: 50 }, size: HANDLE_SIZE },
      { kind: "handle", role: "corner", at: { x: 120, y: 70 }, size: HANDLE_SIZE },
    ]);
  });
  it("maps path handles through the path's own transform when one is given", () => {
    const shapes = overlayShapes(
      input({
        overlay: {
          ...empty,
          handles: [{ point: { x: 1, y: 1 }, kind: "point" }],
          pathAddress: "p",
        },
        mapPath: (p) => ({ x: p.x + 10, y: p.y + 10 }),
      }),
    );
    expect(shapes.find((s) => s.kind === "handle")).toMatchObject({
      at: { x: 122, y: 72 },
      role: "point",
    });
  });
  it("draws in and out handles with a line back to their point", () => {
    const shapes = overlayShapes(
      input({
        overlay: {
          ...empty,
          handles: [
            {
              point: { x: 0, y: 0 },
              kind: "point",
              target: { type: "point", contour: 0, index: 0 },
            },
            { point: { x: 5, y: 0 }, kind: "out", target: { type: "out", contour: 0, index: 0 } },
          ],
        },
      }),
    );
    expect(shapes.filter((s) => s.kind === "line" && s.role === "handle")).toHaveLength(1);
  });
  it("shows the marquee", () => {
    const shapes = overlayShapes(
      input({ overlay: { ...empty, marquee: { x: 0, y: 0, width: 5, height: 5 } } }),
    );
    expect(shapes).toContainEqual({
      kind: "marquee",
      box: { x: 100, y: 50, width: 10, height: 10 },
    });
  });
});

describe("guides and measurements", () => {
  it("draws a vertical guide across the viewport at its coordinate", () => {
    const shapes = overlayShapes(
      input({
        overlay: {
          ...empty,
          guides: [{ axis: "x", kind: "edge", coordinate: 10, targets: [], gap: null }],
        },
      }),
    );
    expect(shapes.find((s) => s.kind === "line")).toMatchObject({
      role: "guide",
      from: { x: 120, y: 0 },
      to: { x: 120, y: 600 },
    });
  });
  it("labels equal spacing with its gap", () => {
    const shapes = overlayShapes(
      input({
        overlay: {
          ...empty,
          guides: [{ axis: "y", kind: "equalSpacing", coordinate: 10, targets: [], gap: 24 }],
        },
      }),
    );
    expect(shapes.find((s) => s.kind === "label")).toMatchObject({
      text: "24 px",
      role: "distance",
    });
  });
  it("labels a drawing preview with its size, whole numbers, below the box", () => {
    const shapes = overlayShapes(
      input({ previewBounds: [{ x: 0, y: 0, width: 100.4, height: 50.6 }] }),
    );
    const label = shapes.find((s) => s.kind === "label");
    expect(label).toMatchObject({ text: "100 × 51", role: "size" });
    expect(label?.kind === "label" && label.at.y).toBeGreaterThan(50 + 50.6 * 2);
    expect(shapes.find((s) => s.kind === "outline")).toMatchObject({ role: "preview" });
  });
  it("labels frames with their names in screen space above the top-left corner", () => {
    const shapes = overlayShapes(
      input({
        frameLabels: [
          { box: { x: 0, y: 0, width: 10, height: 10 }, name: "Step 1", selected: false },
        ],
      }),
    );
    expect(shapes.find((s) => s.kind === "label")).toMatchObject({
      text: "Step 1",
      role: "frame",
      at: { x: 100, y: 50 - 6 },
    });
  });
  it("measures from the selection to the hovered layer on ⌥-hover", () => {
    const sel = { x: 0, y: 0, width: 10, height: 10 };
    const hov = { x: 34, y: 0, width: 10, height: 10 };
    expect(distanceLabels(sel, hov)).toEqual([
      { text: "24 px", from: { x: 10, y: 5 }, to: { x: 34, y: 5 } },
    ]);
    expect(distanceLabels(sel, { x: 0, y: 26, width: 10, height: 10 })).toEqual([
      { text: "16 px", from: { x: 5, y: 10 }, to: { x: 5, y: 26 } },
    ]);
    expect(distanceLabels(sel, { x: 4, y: 4, width: 2, height: 2 })).toEqual([
      { text: "4 px", from: { x: 5, y: 4 }, to: { x: 5, y: 0 } },
      { text: "4 px", from: { x: 5, y: 6 }, to: { x: 5, y: 10 } },
      { text: "4 px", from: { x: 4, y: 5 }, to: { x: 0, y: 5 } },
      { text: "4 px", from: { x: 6, y: 5 }, to: { x: 10, y: 5 } },
    ]);
    const shapes = overlayShapes(
      input({
        altMeasure: true,
        selection: [{ box: sel, kind: "frame", component: false }],
        hover: hov,
      }),
    );
    expect(shapes.filter((s) => s.kind === "label" && s.role === "measure")).toHaveLength(1);
  });
  it("shows no measurements without ⌥ or without a hovered layer", () => {
    const sel = {
      box: { x: 0, y: 0, width: 10, height: 10 },
      kind: "frame",
      component: false,
    } as const;
    expect(
      overlayShapes(
        input({ selection: [sel], hover: { x: 34, y: 0, width: 10, height: 10 } }),
      ).some((s) => s.kind === "label"),
    ).toBe(false);
    expect(
      overlayShapes(input({ selection: [sel], altMeasure: true })).some((s) => s.kind === "label"),
    ).toBe(false);
  });
});

describe("paint handles", () => {
  it("draws the points of a mesh or the corners of a crop where the paint editor put them, in screen pixels", () => {
    const shapes = overlayShapes(
      input({
        paintHandles: [
          { role: "mesh", at: { x: 200, y: 200 }, colour: "#234D3C", selected: true },
          { role: "crop", at: { x: 10, y: 20 }, colour: null, selected: false },
        ],
      }),
    );
    expect(shapes.filter((s) => s.kind === "paintPoint")).toEqual([
      {
        kind: "paintPoint",
        role: "mesh",
        at: { x: 200, y: 200 },
        size: 12,
        colour: "#234D3C",
        selected: true,
      },
      {
        kind: "paintPoint",
        role: "crop",
        at: { x: 10, y: 20 },
        size: HANDLE_SIZE,
        colour: null,
        selected: false,
      },
    ]);
  });
  it("draws them above the selection's own handles", () => {
    const shapes = overlayShapes(
      input({
        overlay: { ...empty, handles: [{ point: { x: 0, y: 0 }, kind: "corner" }] },
        paintHandles: [{ role: "mesh", at: { x: 1, y: 1 }, colour: "#000000", selected: false }],
      }),
    );
    const kinds = shapes.map((s) => s.kind);
    expect(kinds.lastIndexOf("paintPoint")).toBeGreaterThan(kinds.lastIndexOf("handle"));
  });
  it("draws none by default", () => {
    expect(overlayShapes(input()).some((s) => s.kind === "paintPoint")).toBe(false);
  });
});
