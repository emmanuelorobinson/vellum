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
