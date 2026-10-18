// TEST DATA: one rectangle and a scene that records the snap options a drag asks for.
import {
  type EngineSnapshot,
  FakeEngineClient,
  type SnapOptions,
  type ToolScene,
} from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { createEditorSession } from "../session";

const rect = {
  type: "rect",
  id: "a",
  name: "a",
  width: 20,
  height: 20,
  position: { type: "absolute", x: 0, y: 0 },
} as unknown as Node;
const doc = { meta: {}, pages: [{ id: "p", name: "P", children: [rect] }] } as unknown as Document;
function fixture() {
  const asked: Omit<SnapOptions, "zoom">[] = [];
  const scene: ToolScene = {
    get: (id) =>
      id === "a"
        ? { node: rect, bounds: { x: 0, y: 0, width: 20, height: 20 }, children: [], parent: null }
        : undefined,
    roots: () => ["a"],
    hitPoint: () => "a",
    hitRect: () => [],
    snap: (_box, _ids, options) => {
      asked.push(options);
      return { offset: { x: 0, y: 0 }, guides: [] };
    },
    toParent: (_a, p) => p,
    toLocal: (_a, p) => p,
    path: () => null,
  };
  const snapshot: EngineSnapshot = { revision: 0, document: doc, readOnly: false };
  const session = createEditorSession({
    engine: new FakeEngineClient(snapshot, scene),
    device: "dev",
    page: "p",
    nextNodeId: () => "n",
    createNode: () => rect,
    viewportSize: () => ({ width: 800, height: 600 }),
  });
  const drag = () => {
    session.dispatch({ type: "pointerDown", point: { x: 5, y: 5 } });
    session.dispatch({ type: "pointerMove", point: { x: 40, y: 40 } });
    session.dispatch({ type: "pointerUp", point: { x: 40, y: 40 } });
  };
  return { session, asked, drag };
}

describe("canvas snapping from Settings", () => {
  it("snaps to objects and no grid until the person says otherwise", () => {
    const { asked, drag } = fixture();
    drag();
    expect(asked.at(-1)).toMatchObject({
      edges: true,
      centres: true,
      equalSpacing: true,
      grid: null,
    });
  });
  it("turning object snapping off and the pixel grid on changes the next drag only", () => {
    const { session, asked, drag } = fixture();
    session.setSnap({ objects: false, grid: 1 });
    drag();
    expect(asked.at(-1)).toMatchObject({
      edges: false,
      centres: false,
      equalSpacing: false,
      grid: 1,
    });
  });
});
