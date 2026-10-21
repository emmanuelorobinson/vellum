// TEST DATA. Presentation state is local; document writes use the existing transaction boundary.
import { FakeEngineClient } from "@vellum/engine-client";
import type { Document } from "@vellum/types-ts";
import { describe, expect, it, vi } from "vitest";
import { createSlidesController, reorderSlide, slideFrames } from "./slides";

const document = (): Document => ({
  meta: { formatVersion: 1, name: "TEST DATA", createdWith: "fixture" },
  pages: [
    {
      id: "page000001",
      name: "Deck",
      kind: "slides",
      children: [
        { type: "frame", id: "slide00001", name: "One", width: 800, height: 450 },
        {
          type: "note",
          id: "note000001",
          name: "Notes",
          content: "Private",
          width: 100,
          height: 100,
        },
        { type: "frame", id: "slide00002", name: "Two", width: 400, height: 600 },
        { type: "frame", id: "slide00003", name: "Three", width: 640, height: 360 },
      ],
    },
  ],
});
function setup(readOnly = false) {
  const engine = new FakeEngineClient({ document: document(), revision: 0, readOnly });
  let page = "page000001";
  const host = {
    image: vi.fn(async () => "data:image/png;base64,pixels"),
    pdf: vi.fn(async () => {}),
  };
  const c = createSlidesController({
    engine,
    device: "device0001",
    page: () => page,
    nextPageId: () => "page000002",
    onPage: (id) => {
      page = id;
    },
    host,
  });
  return { c, engine, host };
}
describe("slides", () => {
  it("filters annotations and reorders by final child index, including intervening notes", () => {
    const p = document().pages[0]!;
    expect(slideFrames(p).map((s) => s.id)).toEqual(["slide00001", "slide00002", "slide00003"]);
    expect(reorderSlide(p, "slide00001", "slide00002", "after")).toEqual({
      type: "reorder",
      id: "slide00001",
      index: 2,
    });
    expect(reorderSlide(p, "slide00003", "slide00001", "before")).toEqual({
      type: "reorder",
      id: "slide00003",
      index: 0,
    });
    expect(reorderSlide(p, "slide00002", "slide00003", "before")).toBeNull();
    expect(reorderSlide(p, "note000001", "slide00001", "before")).toBeNull();
  });
  it("presents from selected slide, clamps navigation, and never writes or undoes", () => {
    const { c, engine } = setup();
    const before = engine.snapshot();
    const apply = vi.spyOn(engine, "apply");
    expect(c.start("slide00002")).toBe(true);
    expect(c.current()?.id).toBe("slide00002");
    c.newPage();
    c.reorder("slide00003", "slide00001", "before");
    c.next();
    c.next();
    expect(c.current()?.id).toBe("slide00003");
    c.previous();
    c.previous();
    c.previous();
    expect(c.current()?.id).toBe("slide00001");
    c.stop();
    expect(c.state.presenting).toBe(false);
    expect(apply).not.toHaveBeenCalled();
    expect(engine.snapshot()).toEqual(before);
    expect(engine.transactions).toEqual([]);
    expect(engine.historyActors).toEqual([]);
  });
  it("exports selected slides in deck order, rejects annotations and empty selections", async () => {
    const { c, host } = setup(true);
    await c.exportPdf(["slide00003", "slide00001"]);
    expect(host.pdf).toHaveBeenLastCalledWith(["slide00001", "slide00003"], 0);
    await c.exportPdf();
    expect(host.pdf).toHaveBeenLastCalledWith(["slide00001", "slide00002", "slide00003"], 0);
    host.pdf.mockClear();
    await c.exportPdf([]);
    await c.exportPdf(["note000001"]);
    expect(host.pdf).not.toHaveBeenCalled();
    expect(c.state.error).toBeTruthy();
  });
  it("creates slides pages and reorders through attributed Ops; respects read-only", () => {
    const { c, engine } = setup();
    engine.script({ document: document(), revision: 1, readOnly: false });
    c.reorder("slide00003", "slide00001", "before");
    expect(engine.transactions[0]).toEqual({
      origin: { actor: { kind: "person", device: "device0001" } },
      ops: [{ type: "reorder", id: "slide00003", index: 0 }],
    });
    const next = document();
    next.pages.push({ id: "page000002", name: "Slides", kind: "slides", children: [] });
    engine.script({ document: next, revision: 2, readOnly: false });
    c.newPage();
    expect(engine.transactions[1]?.ops[0]).toEqual({
      type: "insertPage",
      index: 1,
      page: next.pages[1],
    });
    const ro = setup(true);
    ro.c.newPage();
    ro.c.reorder("slide00003", "slide00001", "before");
    expect(ro.engine.transactions).toEqual([]);
    expect(ro.c.start()).toBe(true);
  });
  it("stops on removal or page change; missing rendering is explicit", () => {
    const { c, engine } = setup();
    c.start("slide00001");
    const next = document();
    next.pages[0]!.children!.shift();
    engine.script({ document: next, revision: 1, readOnly: false });
    engine.apply({
      origin: { actor: { kind: "person", device: "remote0001" } },
      ops: [{ type: "delete", id: "slide00001" }],
    });
    expect(c.current()?.id).toBe("slide00002");
    next.pages = [];
    engine.script({ document: next, revision: 2, readOnly: false });
    engine.apply({
      origin: { actor: { kind: "person", device: "remote0001" } },
      ops: [{ type: "delete", id: "slide00002" }],
    });
    expect(c.state.presenting).toBe(false);
    c.dispose();
    const no = createSlidesController({
      engine: new FakeEngineClient({ document: document(), revision: 0, readOnly: false }),
      device: "device0001",
      page: () => "page000001",
      nextPageId: () => "page000002",
      onPage: () => {},
    });
    expect(no.start()).toBe(false);
    expect(no.state.error).toContain("unavailable");
    no.dispose();
  });
});

it("shared-export host refuses stale pixels and saves one PDF bundle", async () => {
  const { slidesHostFromExport } = await import("./slides");
  let revision = 1;
  const savePdf = vi.fn(async () => {});
  const render = vi.fn(async (r: { format: string; nodes: readonly string[] }) => ({
    files: [
      {
        name: "export.pdf",
        mime: r.format === "pdf" ? "application/pdf" : "image/png",
        base64: "cGl4ZWxz",
      },
    ],
  }));
  const host = slidesHostFromExport({ revision: () => revision, render, savePdf });
  expect(await host.image!("slide00001", 1)).toBe("data:image/png;base64,cGl4ZWxz");
  await host.pdf!(["slide00002", "slide00001"], 1);
  expect(render).toHaveBeenLastCalledWith({
    format: "pdf",
    nodes: ["slide00002", "slide00001"],
    scale: 1,
  });
  expect(savePdf).toHaveBeenCalledTimes(1);
  render.mockImplementationOnce(async () => {
    revision = 2;
    return { files: [{ name: "stale.pdf", mime: "application/pdf", base64: "cGl4ZWxz" }] };
  });
  await expect(host.pdf!(["slide00001"], 1)).rejects.toThrow("changed");
  expect(savePdf).toHaveBeenCalledTimes(1);
});

it("starts at the slide containing a selected child", () => {
  const doc = document();
  const third = doc.pages[0]?.children?.[3];
  if (third?.type !== "frame") throw Error("fixture frame missing");
  third.children = [
    {
      type: "text",
      id: "child00001",
      name: "TEST DATA",
      width: 100,
      height: 30,
      content: [{ text: "Content", font: "Inter", size: 16 }],
    },
  ];
  const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false });
  const c = createSlidesController({
    engine,
    device: "device0001",
    page: () => "page000001",
    nextPageId: () => "page000002",
    onPage: () => {},
    host: { image: async () => "pixels" },
  });
  c.start("child00001");
  expect(c.current()?.id).toBe("slide00003");
  c.dispose();
});
