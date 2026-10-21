// TEST DATA. Browser import controller has no document write path of its own.
import type { BrowserCapture } from "@vellum/host-api";
import { expect, it, vi } from "vitest";
import { createBrowserImport, embedRequest } from "./index";

const dom = {
  version: 1 as const,
  width: 1280,
  height: 800,
  nodes: [
    {
      kind: "frame" as const,
      name: "article#card",
      selector: "#card",
      parent: null,
      x: 16,
      y: 24,
      width: 200,
      height: 100,
      style: {},
    },
  ],
  notes: [],
};
it("requires a fresh preview, selects a measured element, and forwards imports", async () => {
  const capture = vi.fn(async () => ({ png: "cG5n", width: 1280, height: 800, dom }));
  const importLayers = vi.fn(async () => {});
  const controller = createBrowserImport({ browser: { capture }, importLayers });
  await controller.importLayers();
  expect(importLayers).not.toHaveBeenCalled();
  await controller.load();
  controller.pick(30, 50);
  expect(controller.state.selection?.selector).toBe("#card");
  await controller.importLayers();
  expect(capture.mock.calls).toHaveLength(2);
  expect(importLayers).toHaveBeenCalledOnce();
});
it("discards an obsolete capture after URL changes", async () => {
  let finish!: (value: BrowserCapture) => void;
  const capture = vi.fn(
    () =>
      new Promise<BrowserCapture>((r) => {
        finish = r;
      }),
  );
  const c = createBrowserImport({ browser: { capture } });
  const work = c.load();
  c.configure({ url: "http://localhost:4000/" });
  finish({ png: "old", width: 1, height: 1, dom });
  await work;
  expect(c.state.preview).toBeNull();
  expect(c.state.busy).toBe(false);
});
it("derives embed viewport, zoom and scroll without loading a URL", () => {
  expect(
    embedRequest(
      { url: "http://localhost:3000/", device: "phone", zoom: 0.8, scroll: { x: 0, y: 24 } },
      { width: 300, height: 600 },
    ),
  ).toMatchObject({ width: 375, height: 812, zoom: 0.8, scroll: [0, 24] });
});
