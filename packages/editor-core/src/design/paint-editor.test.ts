// TEST DATA: a frame with a mesh fill, 200 × 100 at (100, 100), shown at 2×.
import { type EngineSnapshot, FakeEngineClient, type ToolScene } from "@vellum/engine-client";
import type { Document, Node, Paint } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { createEditorSession } from "../session";
import { createDesignController } from "./controller";
import { createPaintEditor } from "./paint-editor";

const mesh = {
  type: "mesh",
  columns: 2,
  rows: 2,
  points: [
    { x: 0, y: 0, color: "#234D3C" },
    { x: 1, y: 0, color: "#A5C5A1" },
    { x: 0, y: 1, color: "#E5EDDE" },
    { x: 1, y: 1, color: "#81976E" },
  ],
} as unknown as Paint;
const image = { type: "image", asset: "a1" } as unknown as Paint;

function fixture(fills: Paint[], stroke?: unknown) {
  const node = {
    type: "frame",
    id: "f1",
    name: "Step 2",
    width: 200,
    height: 100,
    position: { type: "absolute", x: 100, y: 100 },
    fills,
    ...(stroke ? { stroke } : {}),
  } as unknown as Node;
  const doc = (): Document =>
    ({
      meta: { formatVersion: 1, name: "t", createdWith: "t" },
      pages: [{ id: "p", name: "P", children: [node] }],
    }) as unknown as Document;
  const scene: ToolScene = {
    get: (id) =>
      id === "f1"
        ? { node, bounds: { x: 100, y: 100, width: 200, height: 100 }, children: [], parent: null }
        : undefined,
    roots: () => ["f1"],
    hitPoint: () => null,
    hitRect: () => [],
    snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
    toParent: (_a, p) => p,
    toLocal: (_a, p) => p,
    path: () => null,
  };
  const snap = (revision: number): EngineSnapshot => ({
    revision,
    document: doc(),
    readOnly: false,
  });
  const engine = new FakeEngineClient(snap(0), scene);
  const session = createEditorSession({
    engine,
    device: "dev1",
    page: "p",
    nextNodeId: () => "n",
    createNode: () => node,
    viewportSize: () => ({ width: 800, height: 600 }),
  });
  session.setViewport({ x: 0, y: 0, zoom: 2 });
  session.select(["f1"]);
  const design = createDesignController({
    session,
    engine,
    defaults: { fill: "#D9D9D9", stroke: "#1F1F1F", shadow: "#00000040" },
    text: { fallback: "Inter" },
  });
  const editor = createPaintEditor({ session, design });
  return {
    engine,
    session,
    editor,
    reply: () => engine.script(snap(engine.snapshot().revision + 1)),
  };
}
const view = { x: 0, y: 0, zoom: 2 };

describe("paint editor: mesh points", () => {
  it("has nothing to show until a paint is opened", () => {
    const { editor } = fixture([mesh]);
    expect(editor.state).toBeNull();
    expect(editor.handles(view)).toEqual([]);
  });
  it("puts one handle on each point, on screen where the point is on the shape", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    const handles = editor.handles(view);
    expect(handles.map((h) => h.at)).toEqual([
      { x: 200, y: 200 },
      { x: 600, y: 200 },
      { x: 200, y: 400 },
      { x: 600, y: 400 },
    ]);
    expect(handles.map((h) => h.colour)).toEqual(["#234D3C", "#A5C5A1", "#E5EDDE", "#81976E"]);
    expect(handles.every((h) => h.role === "mesh")).toBe(true);
  });
  it("selects the point that is pressed", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    expect(editor.press({ x: 602, y: 198 })).toBe(true);
    expect(editor.state?.point).toBe(1);
    expect(editor.handles(view).map((h) => h.selected)).toEqual([false, true, false, false]);
  });
  it("leaves a press that misses every handle to the canvas", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    expect(editor.press({ x: 400, y: 300 })).toBe(false);
  });
  it("moves a point with the pointer, showing it at once, and writes one transaction on release", () => {
    const { editor, engine, reply } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    editor.press({ x: 200, y: 200 });
    expect(editor.drag({ x: 400, y: 300 })).toBe(true);
    expect(editor.handles(view)[0]?.at).toEqual({ x: 400, y: 300 });
    expect(engine.transactions).toHaveLength(0);
    reply();
    expect(editor.release({ x: 400, y: 300 })).toBe(true);
    expect(engine.transactions).toHaveLength(1);
    const op = engine.transactions[0]?.ops[0] as {
      type: string;
      id: string;
      patch: { fills: { points: { x: number; y: number }[] }[] };
    };
    expect(op).toMatchObject({ type: "update", id: "f1" });
    expect(op.patch.fills[0]?.points[0]).toMatchObject({ x: 0.5, y: 0.5 });
    expect(op.patch.fills[0]?.points[1]).toMatchObject({ x: 1, y: 0 });
  });
  it("keeps a dragged point inside the shape", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    editor.press({ x: 600, y: 400 });
    editor.drag({ x: 9000, y: -9000 });
    expect(editor.handles(view)[3]?.at).toEqual({ x: 600, y: 200 });
  });
  it("writes nothing for a press and release without a move", () => {
    const { editor, engine } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    editor.press({ x: 200, y: 200 });
    editor.release({ x: 200, y: 200 });
    expect(engine.transactions).toHaveLength(0);
    expect(editor.state?.point).toBe(0);
  });
  it("puts the point back when the drag is cancelled", () => {
    const { editor, engine } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    expect(editor.cancel()).toBe(false);
    editor.press({ x: 200, y: 200 });
    editor.drag({ x: 400, y: 300 });
    expect(editor.cancel()).toBe(true);
    expect(editor.handles(view)[0]?.at).toEqual({ x: 200, y: 200 });
    expect(engine.transactions).toHaveLength(0);
  });
  it("selects a point from the panel without the canvas", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    editor.selectPoint(3);
    expect(editor.state?.point).toBe(3);
    editor.selectPoint(99);
    expect(editor.state?.point).toBe(3);
  });
  it("works on a stroke's paint too", () => {
    const { editor, engine, reply } = fixture([], { paints: [mesh], width: 1 });
    editor.open({ scope: "stroke", index: 0 });
    expect(editor.handles(view)).toHaveLength(4);
    editor.press({ x: 200, y: 200 });
    editor.drag({ x: 400, y: 300 });
    reply();
    editor.release({ x: 400, y: 300 });
    const op = engine.transactions[0]?.ops[0] as unknown as {
      patch: { stroke: { paints: { points: unknown[] }[] } };
    };
    expect(op.patch.stroke.paints[0]?.points[0]).toMatchObject({ x: 0.5, y: 0.5 });
    expect(op.patch.stroke.paints[0]?.points).toHaveLength(4);
  });
  it("shows nothing for several selected layers, or when the paint is gone", () => {
    const { editor, session } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    session.select([]);
    expect(editor.handles(view)).toEqual([]);
    session.select(["f1"]);
    editor.open({ scope: "fill", index: 4 });
    expect(editor.handles(view)).toEqual([]);
  });
  it("closes", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    editor.close();
    expect(editor.state).toBeNull();
  });
  it("tells subscribers when it changes", () => {
    const { editor } = fixture([mesh]);
    let calls = 0;
    editor.subscribe(() => {
      calls++;
    });
    editor.open({ scope: "fill", index: 0 });
    editor.selectPoint(2);
    editor.close();
    expect(calls).toBe(3);
  });
});

describe("paint editor: image crop", () => {
  it("shows crop handles only while cropping an image", () => {
    const { editor } = fixture([image]);
    editor.open({ scope: "fill", index: 0 });
    expect(editor.handles(view)).toEqual([]);
    editor.setCropping(true);
    expect(editor.handles(view).map((h) => [h.role, h.at])).toEqual([
      ["crop", { x: 200, y: 200 }],
      ["crop", { x: 600, y: 200 }],
      ["crop", { x: 600, y: 400 }],
      ["crop", { x: 200, y: 400 }],
    ]);
  });
  it("cannot crop what is not an image", () => {
    const { editor } = fixture([mesh]);
    editor.open({ scope: "fill", index: 0 });
    editor.setCropping(true);
    expect(editor.state?.cropping).toBe(false);
  });
  it("drags a corner, the opposite one held, and writes the crop on release", () => {
    const { editor, engine, reply } = fixture([image]);
    editor.open({ scope: "fill", index: 0 });
    editor.setCropping(true);
    editor.press({ x: 600, y: 400 });
    editor.drag({ x: 800, y: 500 });
    expect(editor.handles(view)[2]?.at).toEqual({ x: 800, y: 500 });
    reply();
    editor.release({ x: 800, y: 500 });
    expect(engine.transactions[0]?.ops[0]).toMatchObject({
      patch: { fills: [{ type: "image", crop: [1.5, 0, 0, 1.5, 0, 0] }] },
    });
  });
  it("leaves cropping when the paint is closed", () => {
    const { editor } = fixture([image]);
    editor.open({ scope: "fill", index: 0 });
    editor.setCropping(true);
    editor.close();
    editor.open({ scope: "fill", index: 0 });
    expect(editor.state?.cropping).toBe(false);
  });
});
