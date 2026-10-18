import type { Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  createLayerIndex,
  dropTarget,
  layerIcon,
  layerRows,
  planDrop,
  RECENT_AGENT_MS,
  renameTransaction,
  revealPath,
  setLockedTransaction,
  setVisibleTransaction,
} from "./layers";

const node = (type: string, id: string, extra: Record<string, unknown> = {}): Node =>
  ({ type, id, name: id, width: 10, height: 10, ...extra }) as unknown as Node;
const frame = (id: string, children: Node[] = [], extra: Record<string, unknown> = {}) =>
  node("frame", id, { children, ...extra });
const PAGE = { type: "page", id: "p1" } as const;
const device = "device-1";

const tree = () => [
  frame("welcome"),
  frame("step1", [], { layout: "column" }),
  frame(
    "step2",
    [
      node("instance", "card", { componentId: "c1" }),
      frame("list", [node("text", "title"), node("rect", "bar")], { layout: "row" }),
    ],
    {},
  ),
  frame("step3"),
];
const rowsFor = (over: Partial<Parameters<typeof layerRows>[0]> = {}) =>
  layerRows({
    roots: tree(),
    parent: PAGE,
    expanded: new Set(["step2"]),
    selection: [],
    now: 1_000_000,
    ...over,
  });

describe("layer rows", () => {
  it("lists nodes depth-first in document order and hides the children of collapsed layers", () => {
    expect(rowsFor().map((r) => [r.id, r.depth])).toEqual([
      ["welcome", 0],
      ["step1", 0],
      ["step2", 0],
      ["card", 1],
      ["list", 1],
      ["step3", 0],
    ]);
    expect(rowsFor({ expanded: new Set(["step2", "list"]) }).map((r) => r.id)).toContain("title");
  });
  it("can list front to back for people who prefer that order", () => {
    expect(rowsFor({ order: "front-first" }).map((r) => r.id)).toEqual([
      "step3",
      "step2",
      "list",
      "card",
      "step1",
      "welcome",
    ]);
  });
  it("marks containers, expansion and positions among siblings", () => {
    const rows = rowsFor();
    const step2 = rows.find((r) => r.id === "step2");
    expect(step2).toMatchObject({ expandable: true, expanded: true, siblingIndex: 2, siblings: 4 });
    expect(rows.find((r) => r.id === "welcome")).toMatchObject({ expandable: false });
    expect(rows.find((r) => r.id === "list")).toMatchObject({ expandable: true, expanded: false });
  });
  it("uses the design's type icons", () => {
    const icon = (n: Node, componentRoot = false) => layerIcon(n, componentRoot);
    expect(icon(frame("a"))).toBe("frame");
    expect(icon(frame("a", [], { layout: "column" }))).toBe("rows-3");
    expect(icon(frame("a", [], { layout: "row" }))).toBe("columns-3");
    expect(icon(frame("a", [], { layout: "grid" }))).toBe("layout-grid");
    expect(icon(frame("a", [], { slot: { name: "x" } }))).toBe("square-dashed");
    expect(icon(node("text", "a"))).toBe("type");
    expect(icon(node("rect", "a"))).toBe("square");
    expect(icon(node("ellipse", "a"))).toBe("circle");
    expect(icon(node("vector", "a"))).toBe("pen-tool");
    expect(icon(node("image", "a"))).toBe("image");
    expect(icon(node("group", "a"))).toBe("group");
    expect(icon(node("boolean", "a"))).toBe("combine");
    expect(icon(node("icon", "a"))).toBe("shapes");
    expect(icon(node("script", "a"))).toBe("code");
    expect(icon(node("note", "a"))).toBe("sticky-note");
    expect(icon(node("context", "a"))).toBe("scroll-text");
    expect(icon(node("prompt", "a"))).toBe("message-square-play");
    expect(icon(node("embed", "a"))).toBe("app-window");
    expect(icon(node("instance", "a"))).toBe("diamond");
    expect(icon(frame("a"), true)).toBe("component");
  });
  it("tells components and instances apart for the violet treatment", () => {
    const rows = rowsFor({ componentRoots: new Set(["welcome"]) });
    expect(rows.find((r) => r.id === "welcome")).toMatchObject({
      component: true,
      instance: false,
    });
    expect(rows.find((r) => r.id === "card")).toMatchObject({ component: false, instance: true });
  });
  it("reports hidden and locked, and what a layer inherits from its parents", () => {
    const roots = [
      frame("a", [node("rect", "b"), node("rect", "c", { visible: true })], {
        visible: false,
        locked: true,
      }),
    ];
    const rows = layerRows({
      roots,
      parent: PAGE,
      expanded: new Set(["a"]),
      selection: [],
      now: 0,
    });
    expect(rows[0]).toMatchObject({ visible: false, locked: true, hiddenByAncestor: false });
    expect(rows[1]).toMatchObject({
      visible: true,
      locked: false,
      hiddenByAncestor: true,
      lockedByAncestor: true,
    });
  });
  it("shows a variable-bound visibility as bound and not as a plain boolean", () => {
    const rows = layerRows({
      roots: [node("rect", "a", { visible: { $var: "flag" } })],
      parent: PAGE,
      expanded: new Set(),
      selection: [],
      now: 0,
    });
    expect(rows[0]).toMatchObject({ visibleBound: true });
  });
  it("marks the selection", () => {
    expect(
      rowsFor({ selection: ["step1", "card"] })
        .filter((r) => r.selected)
        .map((r) => r.id),
    ).toEqual(["step1", "card"]);
  });
  describe("recent agent marker", () => {
    const recent = {
      step1: { agent: "teal", at: 995_000 },
      step3: { agent: "saffron", at: 989_999 },
    };
    it("shows only for layers created in the last 10 seconds", () => {
      expect(RECENT_AGENT_MS).toBe(10_000);
      const rows = rowsFor({ recent, now: 1_000_000 });
      expect(rows.find((r) => r.id === "step1")?.recentAgent).toBe("teal");
      expect(rows.find((r) => r.id === "step3")?.recentAgent).toBeNull();
      expect(rows.find((r) => r.id === "welcome")?.recentAgent).toBeNull();
    });
    it("is gone exactly at 10 s and never in the future", () => {
      expect(
        rowsFor({ recent: { step1: { agent: "teal", at: 990_000 } } }).find((r) => r.id === "step1")
          ?.recentAgent,
      ).toBeNull();
      expect(
        rowsFor({ recent: { step1: { agent: "teal", at: 990_001 } } }).find((r) => r.id === "step1")
          ?.recentAgent,
      ).toBe("teal");
      expect(
        rowsFor({ recent: { step1: { agent: "teal", at: 2_000_000 } } }).find(
          (r) => r.id === "step1",
        )?.recentAgent,
      ).toBeNull();
    });
  });
});

describe("revealing a selection", () => {
  it("lists the ancestors that must be open to show a nested layer", () => {
    expect(revealPath(tree(), PAGE, "title")).toEqual(["step2", "list"]);
    expect(revealPath(tree(), PAGE, "welcome")).toEqual([]);
    expect(revealPath(tree(), PAGE, "missing")).toBeNull();
  });
});

describe("edits as Person transactions", () => {
  const origin = { actor: { kind: "person", device } };
  it("renames and refuses an empty name", () => {
    expect(renameTransaction(device, "welcome", "  Hero ")).toEqual({
      origin,
      ops: [{ type: "update", id: "welcome", patch: { name: "Hero" } }],
    });
    expect(renameTransaction(device, "welcome", "   ")).toBeNull();
  });
  it("hides by writing false and shows by putting the default back", () => {
    expect(setVisibleTransaction(device, "a", false).ops).toEqual([
      { type: "update", id: "a", patch: { visible: false } },
    ]);
    expect(setVisibleTransaction(device, "a", true).ops).toEqual([
      { type: "update", id: "a", patch: { visible: null } },
    ]);
  });
  it("locks by writing true and unlocks by putting the default back", () => {
    expect(setLockedTransaction(device, "a", true).ops).toEqual([
      { type: "update", id: "a", patch: { locked: true } },
    ]);
    expect(setLockedTransaction(device, "a", false).ops).toEqual([
      { type: "update", id: "a", patch: { locked: null } },
    ]);
  });
});

describe("drag and drop", () => {
  const index = () => createLayerIndex(tree(), PAGE);
  const rows = () => rowsFor();
  const rowIndex = (id: string) => rows().findIndex((r) => r.id === id);

  it("chooses above, below or inside from where the pointer is on a row", () => {
    const base = { rows: rows(), index: index(), dragged: ["step3"] };
    expect(dropTarget({ ...base, over: rowIndex("step1"), offset: 0.1 })).toMatchObject({
      indicator: "above",
      rowIndex: rowIndex("step1"),
      place: { parent: PAGE, index: 1 },
    });
    expect(dropTarget({ ...base, over: rowIndex("step1"), offset: 0.9 })).toMatchObject({
      indicator: "below",
      place: { parent: PAGE, index: 2 },
    });
    expect(dropTarget({ ...base, over: rowIndex("step1"), offset: 0.5 })).toMatchObject({
      indicator: "inside",
      place: { parent: { type: "node", id: "step1" }, index: 0 },
    });
  });
  it("does not drop inside layers that cannot hold children", () => {
    const base = { rows: rows(), index: index(), dragged: ["step3"] };
    expect(dropTarget({ ...base, over: rowIndex("card"), offset: 0.5 })).toBeNull();
    const rect = layerRows({
      roots: [frame("f", [node("rect", "r")]), node("rect", "x")],
      parent: PAGE,
      expanded: new Set(["f"]),
      selection: [],
      now: 0,
    });
    const idx = createLayerIndex([frame("f", [node("rect", "r")]), node("rect", "x")], PAGE);
    expect(dropTarget({ rows: rect, index: idx, dragged: ["x"], over: 1, offset: 0.5 })).toBeNull();
  });
  it("refuses to drop a layer onto itself or into its own subtree", () => {
    const base = { rows: rows(), index: index(), dragged: ["step2"] };
    expect(dropTarget({ ...base, over: rowIndex("step2"), offset: 0.5 })).toBeNull();
    expect(dropTarget({ ...base, over: rowIndex("list"), offset: 0.5 })).toBeNull();
    expect(dropTarget({ ...base, over: rowIndex("card"), offset: 0.1 })).toBeNull();
  });
  it("treats the bottom of an open container as the start of its children", () => {
    const t = dropTarget({
      rows: rows(),
      index: index(),
      dragged: ["welcome"],
      over: rowIndex("step2"),
      offset: 0.9,
    });
    expect(t).toMatchObject({
      indicator: "inside",
      place: { parent: { type: "node", id: "step2" }, index: 0 },
    });
  });
  it("reorders among siblings with one reorder op at the final index", () => {
    const ops = planDrop(index(), ["welcome"], { parent: PAGE, index: 4 });
    expect(ops).toEqual([{ type: "reorder", id: "welcome", index: 3 }]);
    expect(planDrop(index(), ["step3"], { parent: PAGE, index: 0 })).toEqual([
      { type: "reorder", id: "step3", index: 0 },
    ]);
  });
  it("does nothing when a layer is dropped where it already is", () => {
    expect(planDrop(index(), ["step1"], { parent: PAGE, index: 1 })).toEqual([]);
    expect(planDrop(index(), ["step1"], { parent: PAGE, index: 2 })).toEqual([]);
  });
  it("moves a layer into another parent at the requested index", () => {
    expect(
      planDrop(index(), ["welcome"], { parent: { type: "node", id: "list" }, index: 1 }),
    ).toEqual([
      { type: "move", id: "welcome", to: { parent: { type: "node", id: "list" }, index: 1 } },
    ]);
  });
  it("keeps several dragged layers together and in their order", () => {
    // [welcome, step1, step2, step3]; drag welcome and step2 below step3.
    const ops = planDrop(index(), ["welcome", "step2"], { parent: PAGE, index: 4 });
    const order = ["welcome", "step1", "step2", "step3"];
    for (const op of ops) {
      if (op.type !== "reorder") throw Error("expected reorder");
      order.splice(order.indexOf(op.id), 1);
      order.splice(op.index, 0, op.id);
    }
    expect(order).toEqual(["step1", "step3", "welcome", "step2"]);
    expect(ops.every((o) => o.type === "reorder" && ["welcome", "step2"].includes(o.id))).toBe(
      true,
    );
  });
  it("drops dragged layers that are inside other dragged layers", () => {
    const ops = planDrop(index(), ["step2", "list"], { parent: PAGE, index: 0 });
    expect(ops).toEqual([{ type: "reorder", id: "step2", index: 0 }]);
  });
  it("moves several layers from different parents together", () => {
    const ops = planDrop(index(), ["card", "welcome"], { parent: PAGE, index: 4 });
    const moved = ops.map((o) => (o.type === "move" || o.type === "reorder" ? o.id : ""));
    expect(moved.sort()).toEqual(["card", "welcome"]);
  });
});
