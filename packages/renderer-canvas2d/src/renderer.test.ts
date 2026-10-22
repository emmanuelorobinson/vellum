// TEST DATA: compiler boundary behavior; real Canvas2D pixels run in the tagged browser lane.
import { describe, expect, it } from "vitest";
import { createRenderer } from "./index";

describe("basic renderer", () => {
  it("refuses malformed compilation before replacing its previous frame", () => {
    let clears = 0;
    const context = { clearRect: () => clears++, setTransform: () => {}, drawImage: () => {} };
    const canvas = { width: 20, height: 20, getContext: () => context };
    const compile = () => {
      throw new Error("Malformed display list");
    };
    const renderer = createRenderer(canvas as unknown as HTMLCanvasElement, compile);
    expect(() => renderer.draw({ displayList: new Uint8Array(), fonts: [], images: [] })).toThrow(
      "Malformed",
    );
    expect(clears).toBe(0);
    expect(() => renderer.resize({ width: 0, height: 1, dpr: 1 })).toThrow();
    renderer.dispose();
    renderer.dispose();
    expect(renderer.state).toBe("disposed");
  });
});

it("reuses each transferred geometry within a frame and rejects missing references atomically", async () => {
  const { vi } = await import("vitest");
  let built = 0,
    clears = 0;
  vi.stubGlobal(
    "Path2D",
    class {
      constructor(_data: string) {
        built++;
      }
    },
  );
  vi.stubGlobal(
    "OffscreenCanvas",
    class {
      getContext() {
        return { canvas: {}, fillStyle: "", fill() {}, setTransform() {} };
      }
    },
  );
  const canvas = {
    width: 20,
    height: 20,
    getContext: () => ({ clearRect: () => clears++, setTransform() {}, drawImage() {} }),
  };
  let paths = ["M0 0H5V5Z"];
  const renderer = createRenderer(canvas as unknown as HTMLCanvasElement, () => ({
    paths,
    commands: Array.from({ length: 100 }, () => ({
      op: "path" as const,
      data: 0,
      even_odd: false,
      paint: { Solid: [0, 0, 0, 255] as [number, number, number, number] },
      bounds: [0, 0, 5, 5] as [number, number, number, number],
    })),
    simplifications: [],
  }));
  try {
    renderer.draw({ displayList: new Uint8Array(), fonts: [], images: [] });
    expect(built).toBe(1);
    expect(clears).toBe(1);
    paths = [];
    expect(() => renderer.draw({ displayList: new Uint8Array(), fonts: [], images: [] })).toThrow(
      "geometry",
    );
    expect(clears).toBe(1);
  } finally {
    renderer.dispose();
    vi.unstubAllGlobals();
  }
});
