import type { Renderer } from "@vellum/renderer-api";
import { describe, expect, it } from "vitest";
import { rendererContract } from "./renderer";

describe("Renderer contract catches broken backends", () => {
  it("fails a renderer that silently drops drawing commands", () => {
    const renderer: Renderer = {
      state: "ready",
      resize() {},
      draw() {},
      readPixels: () => new Uint8Array(256 * 256 * 4),
      dispose() {},
    };
    expect(() =>
      rendererContract({
        renderer,
        frame: { displayList: new Uint8Array(), fonts: [], images: [] },
      }),
    ).toThrow(/ink/);
  });
});
