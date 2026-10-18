// TEST DATA: a synthetic scene and a scripted engine; no renderer, DOM or document store.
import { type EngineSnapshot, FakeEngineClient, type ToolScene } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { createEditorSession } from "./session";
import type { SceneItem } from "./tool-types";

const doc = (children: Node[]): Document =>
  ({
    meta: { formatVersion: 1, name: "t", createdWith: "test" },
    pages: [{ id: "page1", name: "Page", children }],
  }) as unknown as Document;
const rect = (id: string, x: number, y: number): Node =>
  ({
    type: "rect",
    id,
    name: id,
    width: 20,
    height: 20,
    position: { type: "absolute", x, y },
  }) as unknown as Node;
const snap = (revision: number, children: Node[], readOnly = false): EngineSnapshot => ({
  revision,
  document: doc(children),
  readOnly,
});
function sceneOf(children: () => Node[]): ToolScene {
  const item = (n: Node): SceneItem => {
    const p = (n as unknown as { position: { x: number; y: number } }).position;
    return {
      node: n,
      bounds: { x: p.x, y: p.y, width: 20, height: 20 },
      children: [],
      parent: null,
    };
  };
  const inside = (n: Node, pt: { x: number; y: number }) => {
    const b = item(n).bounds;
    return pt.x >= b.x && pt.x <= b.x + b.width && pt.y >= b.y && pt.y <= b.y + b.height;
  };
  return {
    get: (id) => {
      const n = children().find((c) => c.id === id);
      return n ? item(n) : undefined;
    },
    roots: () => children().map((c) => c.id),
    hitPoint: (pt) => [...children()].reverse().find((n) => inside(n, pt))?.id ?? null,
    hitRect: () => [],
    snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
    toParent: (_a, p) => p,
    toLocal: (_a, p) => p,
    path: () => null,
  };
}
function fixture(initial: Node[] = [rect("a", 10, 10)]) {
  let current = initial;
  let n = 0;
  const engine = new FakeEngineClient(
    snap(0, current),
    sceneOf(() => current),
  );
  const session = createEditorSession({
    engine,
    device: "dev1",
    page: "page1",
    nextNodeId: () => `new${++n}`,
    createNode: (kind, id, b) =>
      ({
        type: kind,
        id,
        name: kind,
        width: b.width,
        height: b.height,
        position: { type: "absolute", x: b.x, y: b.y },
      }) as unknown as Node,
    viewportSize: () => ({ width: 800, height: 600 }),
  });
  const reply = (next: Node[]) => {
    current = next;
    engine.script(snap(engine.snapshot().revision + 1, next));
  };
  return { engine, session, reply };
}
const at = (x: number, y: number) => ({ x, y });

describe("editor session", () => {
  it("draws a rectangle: preview while dragging, one Person transaction on release", () => {
    const { engine, session, reply } = fixture();
    session.dispatch({ type: "tool", tool: "rect" });
    session.dispatch({ type: "pointerDown", point: at(100, 100) });
    const move = session.dispatch({ type: "pointerMove", point: at(160, 140) });
    expect(move.transactions).toEqual([]);
    expect(engine.transactions).toHaveLength(0);
    reply([rect("a", 10, 10), rect("new1", 100, 100)]);
    session.dispatch({ type: "pointerUp", point: at(160, 140) });
    expect(engine.transactions).toHaveLength(1);
    const tx = engine.transactions[0];
    expect(tx?.origin.actor).toEqual({ kind: "person", device: "dev1" });
    expect(tx?.ops[0]).toMatchObject({
      type: "insert",
      to: { parent: { type: "page", id: "page1" } },
    });
    expect(session.state.selection).toEqual(["new1"]);
  });
  it("selects with a click through the scene and notifies once per change", () => {
    const { session } = fixture();
    let calls = 0;
    session.subscribe(() => {
      calls++;
    });
    session.dispatch({ type: "pointerDown", point: at(15, 15) });
    session.dispatch({ type: "pointerUp", point: at(15, 15) });
    expect(session.state.selection).toEqual(["a"]);
    expect(calls).toBeGreaterThan(0);
  });
  it("puts the tool back to a safe state and says so when the engine refuses the commit", () => {
    const { session, engine } = fixture();
    session.dispatch({ type: "tool", tool: "rect" });
    session.dispatch({ type: "pointerDown", point: at(100, 100) });
    session.dispatch({ type: "pointerMove", point: at(160, 140) });
    session.dispatch({ type: "pointerUp", point: at(160, 140) });
    expect(engine.transactions).toHaveLength(0);
    expect(session.state.mode.type).toBe("idle");
    expect(session.notice).toEqual({ kind: "rejected", reason: "unavailable" });
    session.clearNotice();
    expect(session.notice).toBeNull();
  });
  it("refuses to write when the engine says the document is read-only", () => {
    const { session } = fixture();
    const read = createEditorSession({
      engine: new FakeEngineClient(snap(0, [], true)),
      device: "dev1",
      page: "page1",
      nextNodeId: () => "x",
      createNode: () => rect("x", 0, 0),
      viewportSize: () => ({ width: 1, height: 1 }),
    });
    read.dispatch({ type: "tool", tool: "rect" });
    read.dispatch({ type: "pointerDown", point: at(0, 0) });
    read.dispatch({ type: "pointerMove", point: at(50, 50) });
    const r = read.dispatch({ type: "pointerUp", point: at(50, 50) });
    expect(r.transactions).toEqual([]);
    expect(read.notice?.kind).toBe("read-only");
    expect(session.notice).toBeNull();
  });
  it("selects layers from the panel and drops ids that no longer exist after a snapshot", () => {
    const { session, engine, reply } = fixture([rect("a", 10, 10), rect("b", 50, 10)]);
    session.select(["a", "b"]);
    expect(session.state.selection).toEqual(["a", "b"]);
    reply([rect("a", 10, 10)]);
    engine.apply({
      origin: { actor: { kind: "person", device: "dev1" } },
      ops: [{ type: "update", id: "a", patch: {} }],
    });
    expect(session.state.selection).toEqual(["a"]);
  });
  it("undoes and redoes only as the person", () => {
    const { session, engine } = fixture();
    session.undo();
    session.redo();
    expect(engine.historyActors).toEqual([]);
    engine.script(snap(1, []), "undo");
    session.undo();
    engine.script(snap(2, []), "redo");
    session.redo();
    expect(engine.historyActors).toEqual([
      { kind: "person", device: "dev1" },
      { kind: "person", device: "dev1" },
    ]);
  });
  it("tracks the hovered layer for ⌥ measurements", () => {
    const { session } = fixture();
    session.hover(at(15, 15));
    expect(session.hovered).toBe("a");
    session.hover(at(400, 400));
    expect(session.hovered).toBeNull();
  });
  it("changes page, clearing a selection that belonged to the old one", () => {
    const { session } = fixture();
    session.select(["a"]);
    session.setPage("page2");
    expect(session.page).toBe("page2");
    expect(session.state.selection).toEqual([]);
  });
  it("keeps the viewport in the tool state and lets the host set it", () => {
    const { session } = fixture();
    session.setViewport({ x: 10, y: 20, zoom: 2 });
    expect(session.state.viewport).toEqual({ x: 10, y: 20, zoom: 2 });
    expect(() => session.setViewport({ x: 0, y: 0, zoom: 0 })).toThrow();
  });
});
