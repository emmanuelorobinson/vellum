// TEST DATA: synthetic scene and device; no host, renderer or document store.
import type { Node, Point } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  initialToolState,
  type SceneItem,
  TOOLS,
  type ToolContext,
  type ToolEvent,
  type ToolId,
  transitionTool,
} from "./index";

const p = (x: number, y: number): Point => ({ x, y });
function fixture() {
  let next = 0;
  const node = (id: string, type = "rect"): Node =>
    ({
      id,
      type,
      name: "",
      width: 100,
      height: 80,
      ...(type === "frame" ? { children: [] } : {}),
    }) as Node;
  const items = new Map<string, SceneItem>([
    [
      "frame",
      {
        node: node("frame", "frame"),
        bounds: { x: 0, y: 0, width: 100, height: 80 },
        children: ["rect"],
        parent: null,
      },
    ],
    [
      "rect",
      {
        node: { ...node("rect"), position: { type: "absolute", x: 10, y: 20 } },
        bounds: { x: 10, y: 20, width: 20, height: 10 },
        children: [],
        parent: "frame",
      },
    ],
  ]);
  const calls: unknown[] = [];
  const ctx: ToolContext = {
    device: "fixture-device",
    nextNodeId: () => `node${String(++next).padStart(6, "0")}`,
    placement: { parent: { type: "page", id: "fixture-page" }, index: 1 },
    viewportSize: { width: 800, height: 600 },
    dragThreshold: 3,
    snap: {
      search: { x: -1000, y: -1000, width: 2000, height: 2000 },
      tolerance: 6,
      edges: true,
      centres: true,
      equalSpacing: true,
      grid: null,
    },
    createNode: (kind, id, box) =>
      ({
        ...node(id, kind),
        position: { type: "absolute", x: box.x, y: box.y },
        width: box.width,
        height: box.height,
        ...(kind === "text" ? { content: [{ text: "", font: "fixture-font", size: 16 }] } : {}),
        ...(kind === "note" ? { content: "" } : {}),
        ...(kind === "polygon" ? { sides: 3 } : {}),
        ...(kind === "star" ? { points: 5, innerRatio: 0.5 } : {}),
        ...(kind === "vector" ? { path: "", viewBox: box } : {}),
      }) as Node,
    scene: {
      get: (id) => items.get(id),
      roots: () => ["frame"],
      hitPoint: (_point, depth) => (depth === "deepest" ? "rect" : "frame"),
      hitRect: (box, depth) => {
        calls.push({ marquee: box, depth });
        return ["rect"];
      },
      snap: (box, exclude, options) => {
        calls.push({ box, exclude, options });
        return {
          offset: p(1, 2),
          guides: [
            { axis: "x", kind: "edge", coordinate: box.x + 1, targets: ["frame"], gap: null },
          ],
        };
      },
      toParent: (_id, point) => point,
      toLocal: (_id, point) => point,
      path: () => ({
        contours: [
          {
            closed: false,
            points: [
              { point: p(0, 0), in: null, out: null, smooth: false },
              { point: p(50, 0), in: null, out: null, smooth: false },
              { point: p(100, 20), in: null, out: null, smooth: false },
            ],
          },
        ],
      }),
    },
  };
  let state = initialToolState();
  const send = (event: ToolEvent) => {
    const result = transitionTool(state, event, ctx);
    state = result.state;
    return result;
  };
  const tool = (tool: ToolId) => send({ type: "tool", tool });
  const down = (x: number, y: number, mods = {}) =>
    send({ type: "pointerDown", point: p(x, y), ...mods });
  const move = (x: number, y: number, mods = {}) =>
    send({ type: "pointerMove", point: p(x, y), ...mods });
  const up = (x: number, y: number, mods = {}) =>
    send({ type: "pointerUp", point: p(x, y), ...mods });
  const key = (key: string, mods = {}) => send({ type: "keyDown", key, ...mods });
  return { ctx, items, calls, send, tool, down, move, up, key, state: () => state };
}
describe("selection and transforms", () => {
  it("clicks top level, command clicks deepest, shift adds, Enter navigates and Escape clears", () => {
    const f = fixture();
    f.down(10, 10);
    f.up(10, 10);
    expect(f.state().selection).toEqual(["frame"]);
    f.key("Enter");
    expect(f.state().selection).toEqual(["rect"]);
    f.key("Enter", { shift: true });
    expect(f.state().selection).toEqual(["frame"]);
    f.down(10, 10, { command: true, shift: true });
    f.up(10, 10, { command: true, shift: true });
    expect(f.state().selection).toEqual(["frame", "rect"]);
    f.key("Escape");
    expect(f.state().selection).toEqual([]);
  });
  it("command drag selects inside frames without moving the hit layer", () => {
    const f = fixture();
    f.down(5, 5, { command: true });
    expect(f.move(70, 60, { command: true }).transactions).toEqual([]);
    const r = f.up(70, 60, { command: true });
    expect(r.transactions).toEqual([]);
    expect(f.state().selection).toEqual(["rect"]);
    expect(f.calls).toContainEqual({
      marquee: { x: 5, y: 5, width: 65, height: 55 },
      depth: "deepest",
    });
  });
  it("previews snapped movement then commits once, excluding descendants and converting coordinates", () => {
    const f = fixture();
    f.ctx.scene.toParent = (_id, point) => p(point.y, -point.x);
    f.down(10, 10);
    const r = f.move(20, 30);
    expect(r.transactions).toEqual([]);
    expect(r.overlay.guides).toHaveLength(1);
    expect(r.preview[0]).toMatchObject({
      type: "update",
      id: "frame",
      patch: { position: { type: "absolute", x: 22, y: -11 } },
    });
    const commit = f.up(20, 30);
    expect(commit.transactions).toEqual([
      { origin: { actor: { kind: "person", device: "fixture-device" } }, ops: r.preview },
    ]);
    expect(f.up(20, 30).transactions).toEqual([]);
    expect(f.state().mode.type).toBe("idle");
  });
  it("does not double transform a selected descendant; cancel discards preview", () => {
    const f = fixture();
    f.down(1, 1);
    f.up(1, 1);
    f.down(1, 1, { command: true, shift: true });
    f.up(1, 1, { command: true, shift: true });
    f.down(1, 1);
    const r = f.move(10, 10);
    expect(r.preview).toHaveLength(1);
    expect(f.key("Escape").transactions).toEqual([]);
    expect(f.state().mode.type).toBe("idle");
    expect(f.state().selection).toEqual(["frame", "rect"]);
  });
  it("K previews scale with a normalized anchor; Enter commits a Scale op and returns to Move", () => {
    const f = fixture();
    f.down(1, 1);
    f.up(1, 1);
    f.key("k");
    f.send({ type: "scaleAnchor", anchor: p(1, 0.5) });
    const r = f.send({ type: "scaleFactor", factor: "200%" });
    expect(r.transactions).toEqual([]);
    expect(r.preview).toEqual([{ type: "scale", id: "frame", factor: 2, anchor: p(1, 0.5) }]);
    expect(f.key("Enter").transactions[0]?.ops).toEqual(r.preview);
    expect(f.state().tool).toBe("move");
  });
  it("scale corner drag is proportional and commits only on release", () => {
    const f = fixture();
    f.down(1, 1);
    f.up(1, 1);
    f.tool("scale");
    f.down(100, 80, { scaleHandle: p(1, 1) });
    expect(f.move(200, 160).preview[0]).toMatchObject({ type: "scale", factor: 2 });
    expect(f.up(200, 160).transactions).toHaveLength(1);
  });
  it("rejects invalid scale input without losing the previous preview", () => {
    const f = fixture();
    f.tool("scale");
    for (const factor of ["0", "-1", "NaN", "1junk", "Infinity"])
      expect(f.send({ type: "scaleFactor", factor }).error).toBe("invalid-input");
    expect(f.send({ type: "scaleAnchor", anchor: p(2, 0) }).error).toBe("invalid-input");
  });
});
describe("creation and content", () => {
  for (const kind of ["frame", "rect", "ellipse", "line", "polygon", "star"] as const) {
    it(`${kind} previews and inserts one schema-shaped node`, () => {
      const f = fixture();
      f.tool(kind);
      f.down(20, 30);
      const r = f.move(80, 70);
      expect(r.transactions).toEqual([]);
      const c = f.up(80, 70);
      expect(c.transactions).toHaveLength(1);
      expect(c.transactions[0]?.ops[0]).toMatchObject({
        type: "insert",
        node: { type: kind },
        to: f.ctx.placement,
      });
      expect(f.state().tool).toBe("move");
    });
  }
  it("line follows the drag direction, not the rectangle diagonal", () => {
    const f = fixture();
    f.tool("line");
    f.down(80, 70);
    f.up(20, 30);
    expect(f.state().selection).toHaveLength(1);
  });
  for (const kind of ["text", "note"] as const) {
    it(`${kind} drafts content locally and commits on explicit finish`, () => {
      const f = fixture();
      f.tool(kind);
      f.down(10, 20);
      expect(f.up(80, 50).transactions).toEqual([]);
      const r = f.send({
        type: "content",
        content: kind === "text" ? [{ text: "hello", font: "fixture-font", size: 16 }] : "hello",
      });
      expect(r.transactions).toEqual([]);
      expect(r.preview[0]).toMatchObject({ type: "insert", node: { type: kind } });
      expect(f.send({ type: "finish" }).transactions).toHaveLength(1);
    });
    it(`${kind} cancels without inserting an empty draft`, () => {
      const f = fixture();
      f.tool(kind);
      f.down(10, 20);
      f.up(10, 20);
      expect(f.key("Escape").transactions).toEqual([]);
      expect(f.state().mode.type).toBe("idle");
    });
  }
  it("edits existing rich text without replacing its other fields", () => {
    const f = fixture();
    required(f.items.get("rect")).node = {
      type: "text",
      id: "rect",
      name: "",
      width: 100,
      height: 80,
      content: [{ text: "old", font: "fixture-font", size: 16 }],
    };
    f.tool("text");
    f.down(10, 10);
    f.up(10, 10);
    const content = [{ text: "new", font: "fixture-font", size: 16, underline: true }];
    f.send({ type: "content", content });
    expect(f.send({ type: "finish" }).transactions[0]?.ops).toEqual([
      { type: "update", id: "rect", patch: { content } },
    ]);
  });
  it("pen builds cubic handles without a transaction per pointer move", () => {
    const f = fixture();
    f.tool("pen");
    f.down(0, 0);
    f.move(10, 0);
    f.up(10, 0);
    f.down(100, 20);
    f.move(110, 20);
    f.up(110, 20);
    const r = f.key("Enter");
    expect(r.transactions).toHaveLength(1);
    expect(r.transactions[0]?.ops[0]).toMatchObject({
      type: "insert",
      node: { type: "vector", path: expect.stringContaining("C") },
    });
  });
});
describe("path editing", () => {
  const begin = () => {
    const f = fixture();
    f.down(1, 1, { command: true });
    f.up(1, 1, { command: true });
    f.key("Enter");
    expect(f.state().mode.type).toBe("path");
    return f;
  };
  it("selects points, selects all, and finishes with Enter or Escape", () => {
    for (const key of ["Enter", "Escape"]) {
      const f = begin();
      f.key("a", { command: true });
      const mode = f.state().mode;
      expect(mode.type === "path" && mode.selected).toHaveLength(3);
      expect(f.key(key).transactions).toEqual([]);
      expect(f.state().mode.type).toBe("idle");
    }
  });
  it("symmetric point drag previews, commits once, and retains the path session", () => {
    const f = begin();
    f.down(0, 0, { target: { type: "point", contour: 0, index: 0 }, command: true });
    expect(f.move(10, 10, { command: true }).transactions).toEqual([]);
    const r = f.up(10, 10, { command: true });
    expect(r.transactions).toHaveLength(1);
    expect(r.transactions[0]?.ops[0]).toMatchObject({
      type: "replace",
      id: "rect",
      node: { type: "vector", path: expect.stringContaining("C") },
    });
    expect(f.state().mode.type).toBe("path");
  });
  for (const action of [
    "toggleSmooth",
    "cut",
    "connect",
    "straighten",
    "addPoint",
    "delete",
  ] as const) {
    it(`performs ${action} through an attributed transaction`, () => {
      const f = begin();
      const r = f.send({
        type: "pathAction",
        action,
        target: {
          type:
            action === "toggleSmooth" || action === "delete" || action === "connect"
              ? "point"
              : "segment",
          contour: 0,
          index: action === "connect" ? 2 : 1,
        },
        other: { type: "point", contour: 0, index: 0 },
        t: 0.5,
      });
      expect(r.transactions).toHaveLength(1);
      expect(r.transactions[0]?.origin.actor.kind).toBe("person");
    });
  }
  it("bends a segment while preserving its neighbours; Escape rolls back only the drag", () => {
    const f = begin();
    f.down(25, 0, {
      command: true,
      alt: true,
      target: { type: "segment", contour: 0, index: 0, t: 0.5 },
    });
    const r = f.move(25, 10, { command: true, alt: true });
    expect(r.transactions).toEqual([]);
    expect(r.overlay.handles.length).toBeGreaterThan(0);
    expect(f.key("Escape").transactions).toEqual([]);
    expect(f.state().mode.type).toBe("path");
  });
});
describe("viewport and total transitions", () => {
  it("Space temporarily pans and restores the original tool, even when released mid-drag", () => {
    const f = fixture();
    f.tool("rect");
    f.key(" ");
    f.down(10, 20);
    f.move(50, 80);
    expect(f.state().viewport).toEqual({ x: 40, y: 60, zoom: 1 });
    f.send({ type: "keyUp", key: " " });
    expect(f.state().tool).toBe("hand");
    expect(f.up(50, 80).transactions).toEqual([]);
    expect(f.state().tool).toBe("rect");
  });
  it("held Z click zooms around the cursor; Alt zooms out; region fits viewport", () => {
    const f = fixture();
    f.key("z");
    f.down(100, 100);
    f.up(100, 100);
    expect(f.state().viewport).toEqual({ x: -100, y: -100, zoom: 2 });
    f.down(100, 100, { alt: true });
    f.up(100, 100, { alt: true });
    expect(f.state().viewport.zoom).toBe(1);
    f.down(0, 0);
    f.up(400, 300);
    expect(f.state().viewport.zoom).toBe(2);
    f.send({ type: "keyUp", key: "z" });
    expect(f.state().tool).toBe("move");
  });
  it("does not interpret tool shortcuts while entering text or outside canvas focus", () => {
    const f = fixture();
    f.key("r", { canvasFocus: false });
    expect(f.state().tool).toBe("move");
    f.tool("text");
    f.down(0, 0);
    f.up(0, 0);
    f.key("v");
    expect(f.state().tool).toBe("text");
  });
  for (const tool of TOOLS) {
    it(`${tool} handles release/cancel/finish/tool-switch without stray transactions`, () => {
      const f = fixture();
      f.tool(tool);
      expect(f.up(0, 0).transactions).toEqual([]);
      f.down(0, 0);
      f.move(20, 20);
      const cancelled = f.send({ type: "cancel" });
      expect(cancelled.transactions).toEqual([]);
      expect(cancelled.preview).toEqual([]);
      expect(cancelled.overlay.guides).toEqual([]);
      expect(f.up(20, 20).transactions).toEqual([]);
      expect(f.send({ type: "finish" }).transactions).toEqual([]);
      f.tool("move");
      expect(f.state().tool).toBe("move");
    });
  }
  it("is deterministic, leaves its inputs untouched and refuses non-finite pointer data", () => {
    const f = fixture();
    const state = initialToolState();
    const event: ToolEvent = { type: "pointerMove", point: p(Number.NaN, 0) };
    expect(transitionTool(state, event, f.ctx).error).toBe("invalid-input");
    const before = JSON.stringify(state);
    const e: ToolEvent = { type: "keyDown", key: "k" };
    expect(transitionTool(state, e, f.ctx)).toEqual(transitionTool(state, e, f.ctx));
    expect(JSON.stringify(state)).toBe(before);
  });
});
describe("gesture boundaries and engine addresses", () => {
  it("pen commits a visible vector with a positive viewBox containing every point", () => {
    const f = fixture();
    f.tool("pen");
    f.down(20, 30);
    f.up(20, 30);
    f.down(120, 80);
    f.up(120, 80);
    const op = f.key("Enter").transactions[0]?.ops[0];
    expect(op?.type).toBe("insert");
    if (op?.type !== "insert" || op.node.type !== "vector")
      throw new Error("expected vector insert");
    expect(op.node.width).toBeGreaterThan(0);
    expect(op.node.height).toBeGreaterThan(0);
    expect(op.node.viewBox.width).toBeGreaterThan(0);
    expect(op.node.viewBox.height).toBeGreaterThan(0);
    expect(op.node.position).toEqual({ type: "absolute", x: 21, y: 32 });
  });
  it("pointer double-clicks toggle smooth points and command straightens segments", () => {
    const f = fixture();
    f.down(0, 0, { command: true });
    f.up(0, 0, { command: true });
    f.key("Enter");
    const point = { type: "point", contour: 0, index: 1 } as const;
    expect(
      f.send({ type: "doubleClick", point: p(50, 0), target: point }).transactions,
    ).toHaveLength(1);
    const segment = { type: "segment", contour: 0, index: 0 } as const;
    const r = f.send({ type: "doubleClick", point: p(25, 0), target: segment, command: true });
    expect(r.transactions).toHaveLength(1);
    expect(f.state().mode.type).toBe("path");
  });
  it("Space suspends the path session and restores it after panning", () => {
    const f = fixture();
    f.down(0, 0, { command: true });
    f.up(0, 0, { command: true });
    f.key("Enter");
    const before = f.state().mode;
    f.key(" ");
    expect(f.state().tool).toBe("hand");
    f.down(10, 10);
    f.up(30, 20);
    f.send({ type: "keyUp", key: " " });
    expect(f.state().mode).toEqual(before);
    expect(f.state().tool).toBe("path");
    expect(f.state().viewport).toEqual({ x: 20, y: 10, zoom: 1 });
  });
  it("instance descendant movement creates an override and never updates its definition id", () => {
    const f = fixture();
    required(f.items.get("rect")).instance = { id: "instance", path: "definition/child" };
    f.down(0, 0, { command: true });
    f.up(0, 0, { command: true });
    f.ctx.scene.hitPoint = () => "rect";
    f.down(0, 0);
    const r = f.up(20, 20);
    expect(r.transactions[0]?.ops[0]).toMatchObject({
      type: "replaceInInstance",
      id: "instance",
      path: "definition/child",
      node: { id: "rect" },
    });
  });
  it("missing selected geometry yields a typed error, including after opening the Scale tool", () => {
    const f = fixture();
    f.down(0, 0);
    f.up(0, 0);
    f.tool("scale");
    f.items.delete("frame");
    expect(f.send({ type: "scaleFactor", factor: 2 }).error).toBe("missing-data");
    expect(f.key("Enter").error).toBe("missing-data");
  });
  it("read-only documents can select and navigate but cannot commit edits", () => {
    const f = fixture();
    f.ctx.readOnly = true;
    f.down(0, 0);
    f.up(0, 0);
    expect(f.state().selection).toEqual(["frame"]);
    f.down(0, 0);
    const r = f.up(20, 20);
    expect(r.error).toBe("read-only");
    expect(r.transactions).toEqual([]);
    f.tool("rect");
    expect(f.down(0, 0).error).toBe("read-only");
  });
  it("unfocused key-up still releases a held viewport tool", () => {
    const f = fixture();
    f.key("z");
    f.send({ type: "keyUp", key: "z", canvasFocus: false });
    expect(f.state().tool).toBe("move");
  });
});
describe("state × event transition matrix", () => {
  const preparations: [string, (f: ReturnType<typeof fixture>) => void][] = [
    ["idle", () => {}],
    [
      "pressed",
      (f) => {
        f.down(0, 0);
      },
    ],
    [
      "dragging",
      (f) => {
        f.down(0, 0);
        f.move(20, 20);
      },
    ],
    [
      "marquee",
      (f) => {
        f.down(0, 0, { command: true });
        f.move(20, 20, { command: true });
      },
    ],
    [
      "creating",
      (f) => {
        f.tool("rect");
        f.down(0, 0);
      },
    ],
    [
      "editing",
      (f) => {
        f.tool("text");
        f.down(0, 0);
        f.up(20, 20);
      },
    ],
    [
      "pen",
      (f) => {
        f.tool("pen");
        f.down(0, 0);
        f.up(0, 0);
      },
    ],
    [
      "path",
      (f) => {
        f.down(0, 0, { command: true });
        f.up(0, 0, { command: true });
        f.key("Enter");
      },
    ],
    [
      "path dragging",
      (f) => {
        f.down(0, 0, { command: true });
        f.up(0, 0, { command: true });
        f.key("Enter");
        f.down(0, 0, { target: { type: "point", contour: 0, index: 0 } });
      },
    ],
    [
      "scaling",
      (f) => {
        f.down(0, 0);
        f.up(0, 0);
        f.tool("scale");
      },
    ],
    [
      "panning",
      (f) => {
        f.tool("hand");
        f.down(0, 0);
      },
    ],
    [
      "zooming",
      (f) => {
        f.tool("zoom");
        f.down(0, 0);
      },
    ],
  ];
  preparations.push(
    [
      "pen handle",
      (f) => {
        f.tool("pen");
        f.down(0, 0);
      },
    ],
    [
      "scale handle",
      (f) => {
        f.down(0, 0);
        f.up(0, 0);
        f.tool("scale");
        f.down(100, 80, { scaleHandle: p(1, 1) });
      },
    ],
    [
      "path marquee",
      (f) => {
        f.down(0, 0, { command: true });
        f.up(0, 0, { command: true });
        f.key("Enter");
        f.down(-1, -1);
      },
    ],
  );
  const events: ToolEvent[] = [
    { type: "pointerDown", point: p(20, 20) },
    { type: "pointerMove", point: p(30, 30) },
    { type: "pointerUp", point: p(30, 30) },
    { type: "doubleClick", point: p(30, 30), target: { type: "point", contour: 0, index: 0 } },
    { type: "keyDown", key: "Escape" },
    { type: "keyDown", key: "Enter" },
    { type: "keyDown", key: "q" },
    { type: "keyDown", key: "a", command: true },
    { type: "keyUp", key: "z" },
    { type: "cancel" },
    { type: "finish" },
    { type: "tool", tool: "rect" },
    { type: "scaleFactor", factor: 2 },
    { type: "scaleAnchor", anchor: p(0, 0) },
    { type: "content", content: [{ text: "draft", font: "fixture-font", size: 16 }] },
    { type: "pathAction", action: "toggleSmooth", target: { type: "point", contour: 0, index: 0 } },
  ];
  for (const [label, prepare] of preparations)
    for (const event of events) {
      it(`${label} → ${event.type}${"key" in event ? ` ${event.key}` : ""} is pure and has at most one commit`, () => {
        const f = fixture();
        prepare(f);
        const before = JSON.stringify(f.state());
        const r = transitionTool(f.state(), event, f.ctx);
        expect(JSON.stringify(f.state())).toBe(before);
        expect(TOOLS).toContain(r.state.tool);
        expect(r.transactions.length).toBeLessThanOrEqual(1);
        for (const tx of r.transactions) {
          expect(tx.origin.actor).toEqual({ kind: "person", device: f.ctx.device });
          expect(tx.ops.length).toBeGreaterThan(0);
        }
        if (
          ["pointerMove", "cancel", "scaleFactor", "scaleAnchor", "content", "keyUp"].includes(
            event.type,
          )
        )
          expect(r.transactions).toEqual([]);
        if (event.type === "cancel" || event.type === "tool") {
          expect(r.preview).toEqual([]);
          expect(r.overlay.guides).toEqual([]);
        }
      });
    }
});
describe("path point marquee and click defaults", () => {
  it("path selection box selects points without a document write", () => {
    const f = fixture();
    f.down(0, 0, { command: true });
    f.up(0, 0, { command: true });
    f.key("Enter");
    f.down(-1, -1);
    const preview = f.move(60, 10);
    expect(preview.transactions).toEqual([]);
    expect(preview.overlay.marquee).not.toBeNull();
    const r = f.up(60, 10);
    expect(r.transactions).toEqual([]);
    const mode = f.state().mode;
    expect(mode.type === "path" && mode.selected.map((t) => t.index)).toEqual([0, 1]);
  });
  it("click-created text keeps the host's supplied dimensions", () => {
    const f = fixture(),
      create = f.ctx.createNode;
    f.ctx.createNode = (kind, id, box) => ({ ...create(kind, id, box), width: 240, height: 120 });
    f.tool("text");
    f.down(20, 20);
    f.up(20, 20);
    const mode = f.state().mode;
    expect(mode.type === "editing" && [mode.node.width, mode.node.height]).toEqual([240, 120]);
  });
});
describe("path segment modifier rules", () => {
  it("selects a segment without bending until Command is held", () => {
    const f = fixture();
    f.down(0, 0, { command: true });
    f.up(0, 0, { command: true });
    f.key("Enter");
    f.down(25, 0, { target: { type: "segment", contour: 0, index: 0 } });
    expect(f.move(25, 20).preview).toEqual([]);
    expect(f.up(25, 20).transactions).toEqual([]);
    expect(f.key("x").transactions).toHaveLength(1);
  });
});

function required<T>(value: T | null | undefined): T {
  if (value == null) throw new Error("Missing fixture value");
  return value;
}

describe("resolved primitive geometry", () => {
  it("converts a bound primitive using the engine's viewBox and retains its size binding", () => {
    const f = fixture(),
      item = required(f.items.get("rect")),
      getPath = f.ctx.scene.path;
    item.node = { ...item.node, width: { $var: "fixture-width" }, height: "hug" };
    f.ctx.scene.path = (id) => {
      const path = getPath(id);
      return path ? { ...path, viewBox: { x: 0, y: 0, width: 140, height: 80 } } : null;
    };
    f.down(0, 0, { command: true });
    f.up(0, 0, { command: true });
    f.key("Enter");
    const r = f.send({
      type: "pathAction",
      action: "toggleSmooth",
      target: { type: "point", contour: 0, index: 1 },
    });
    expect(r.transactions[0]?.ops[0]).toMatchObject({
      type: "replace",
      node: {
        type: "vector",
        width: { $var: "fixture-width" },
        height: "hug",
        viewBox: { x: 0, y: 0, width: 140, height: 80 },
      },
    });
  });
});

describe("double-clicking text", () => {
  const textItem = (): SceneItem => ({
    node: {
      id: "txt",
      type: "text",
      name: "Title",
      width: 100,
      height: 24,
      content: [{ text: "Hello", font: "Inter", size: 16 }],
    } as unknown as Node,
    bounds: { x: 0, y: 0, width: 100, height: 24 },
    children: [],
    parent: null,
  });
  const over = (item: SceneItem) => {
    const f = fixture();
    const ctx: ToolContext = {
      ...f.ctx,
      scene: {
        ...f.ctx.scene,
        hitPoint: () => item.node.id,
        get: (id) => (id === item.node.id ? item : f.ctx.scene.get(id)),
      },
    };
    return { ctx, state: initialToolState() };
  };
  it("opens the text layer for typing with the move tool, keeping what it says", () => {
    const { ctx, state } = over(textItem());
    const r = transitionTool(state, { type: "doubleClick", point: p(10, 10) }, ctx);
    expect(r.state.mode).toMatchObject({
      type: "editing",
      address: "txt",
      original: [{ text: "Hello", font: "Inter", size: 16 }],
    });
    expect(r.state.selection).toEqual(["txt"]);
    expect(r.transactions).toEqual([]);
  });
  it("opens text inside an instance too: the change is then an override of the instance", () => {
    const inner = { ...textItem(), instance: { id: "inst", path: "inst/txt" } };
    const { ctx, state } = over(inner);
    const r = transitionTool(state, { type: "doubleClick", point: p(10, 10) }, ctx);
    expect(r.state.mode.type).toBe("editing");
    const typed = transitionTool(
      r.state,
      { type: "content", content: [{ text: "Bye", font: "Inter", size: 16 }] },
      ctx,
    );
    const done = transitionTool(typed.state, { type: "finish" }, ctx);
    expect(done.transactions[0]?.ops[0]).toMatchObject({
      type: "replaceInInstance",
      id: "inst",
      path: "inst/txt",
    });
  });
  it("does nothing to a layer that is not text", () => {
    const f = fixture();
    const r = transitionTool(initialToolState(), { type: "doubleClick", point: p(10, 10) }, f.ctx);
    expect(r.state.mode.type).toBe("idle");
  });
  it("does not open a locked or hidden text layer, or one in a read-only file", () => {
    const locked = textItem();
    (locked.node as unknown as { locked: boolean }).locked = true;
    const a = over(locked);
    expect(
      transitionTool(a.state, { type: "doubleClick", point: p(1, 1) }, a.ctx).state.mode.type,
    ).toBe("idle");
    const b = over(textItem());
    expect(
      transitionTool(b.state, { type: "doubleClick", point: p(1, 1) }, { ...b.ctx, readOnly: true })
        .state.mode.type,
    ).toBe("idle");
  });
  it("opens only while the tool is idle: a press in progress is left alone", () => {
    const { ctx, state } = over(textItem());
    const pressed = transitionTool(state, { type: "pointerDown", point: p(5, 5) }, ctx).state;
    expect(
      transitionTool(pressed, { type: "doubleClick", point: p(5, 5) }, ctx).state.mode.type,
    ).toBe("pressed");
  });
});
