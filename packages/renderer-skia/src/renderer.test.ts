import { describe, expect, it } from "vitest";
import { createRenderer } from "./index";

describe("Skia editor adapter", () => {
  it("refuses an ABI mismatch before creating a graphics context", async () => {
    let called = false;
    await expect(
      createRenderer(
        { width: 80, height: 80 } as HTMLCanvasElement,
        async () =>
          ({
            _vellum_display_list_version: () => 99,
            GL: {
              createContext: () => {
                called = true;
              },
            },
          }) as never,
      ),
    ).rejects.toThrow(/version/);
    expect(called).toBe(false);
  });
});

import { rendererContract } from "@vellum/contracts";
import { mockModule } from "./mock-module";

function canvasMock() {
  const listeners = new Map<string, EventListenerOrEventListenerObject>();
  const canvas = {
    width: 256,
    height: 256,
    addEventListener: (n: string, f: EventListenerOrEventListenerObject) => listeners.set(n, f),
    removeEventListener: (n: string) => listeners.delete(n),
  } as unknown as HTMLCanvasElement;
  return {
    canvas,
    fire: (name: string) => {
      const listener = listeners.get(name);
      if (typeof listener === "function") listener({ preventDefault() {} } as Event);
    },
    count: () => listeners.size,
  };
}
describe("Renderer contract and resource lifetime", () => {
  it("runs the port contract through the Skia ABI adapter and frees every transfer", async () => {
    const { module, live } = mockModule(),
      { canvas, count } = canvasMock();
    const renderer = await createRenderer(canvas, async () => module);
    rendererContract({
      renderer,
      frame: { displayList: new Uint8Array([0]), fonts: [], images: [] },
    });
    expect(live()).toBe(0);
    expect(count()).toBe(0);
  });
  it("recovers after context loss and remains disposed after queued restore events", async () => {
    const { module } = mockModule(),
      { canvas, fire } = canvasMock();
    const renderer = await createRenderer(canvas, async () => module);
    fire("webglcontextlost");
    expect(renderer.state).toBe("context-lost");
    expect(() => renderer.draw({ displayList: new Uint8Array(0), fonts: [], images: [] })).toThrow(
      /context-lost/,
    );
    fire("webglcontextrestored");
    expect(renderer.state).toBe("ready");
    renderer.dispose();
    fire("webglcontextrestored");
    expect(renderer.state).toBe("disposed");
  });
  it("rejects out-of-range font handles without allocating", async () => {
    const { module, live } = mockModule(),
      { canvas } = canvasMock();
    const renderer = await createRenderer(canvas, async () => module);
    expect(() =>
      renderer.draw({
        displayList: new Uint8Array(0),
        fonts: [{ handle: 1n << 64n, index: 0, bytes: new Uint8Array(0) }],
        images: [],
      }),
    ).toThrow(/handle/);
    expect(live()).toBe(0);
    renderer.dispose();
  });
});

it("uploads bounded shader resources and reads diagnostics without leaking transfer buffers", async () => {
  const { module, live } = mockModule();
  const { canvas } = canvasMock();
  const renderer = await createRenderer(canvas, async () => module);
  renderer.draw({
    displayList: new Uint8Array([0]),
    fonts: [],
    images: [],
    shaders: [{ id: "checker.vsh", source: "half4 main(float2 p) { return half4(1); }" }],
  });
  expect(renderer.shaderErrors?.()).toEqual([]);
  expect(live()).toBe(0);
  expect(() =>
    renderer.draw({
      displayList: new Uint8Array([0]),
      fonts: [],
      images: [],
      shaders: [{ id: "x", source: "x".repeat(16385) }],
    }),
  ).toThrow(/large/);
  expect(live()).toBe(0);
  renderer.dispose();
});
