// TEST DATA: derived script results and host replies; the engine is scripted, never reimplemented.
import { FakeEngineClient } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  createAuthoringController,
  insertSlotOp,
  parseScriptInput,
  scriptView,
  slotTargets,
} from "./index";

const card = {
  type: "frame",
  id: "root",
  name: "Card",
  width: 360,
  height: 257,
  children: [
    {
      type: "frame",
      id: "slot",
      name: "Content",
      width: "fill",
      height: 168,
      slot: { suggested: ["row"] },
    },
  ],
} as Node;
const doc: Document = {
  meta: { formatVersion: 1, name: "Test", createdWith: "test" },
  pages: [{ id: "page", name: "Page", children: [] }],
  components: {
    card: { name: "Card", root: card as never },
    row: {
      name: "Row",
      root: { type: "frame", id: "rowroot", name: "Row", width: 100, height: 32 } as never,
    },
  },
};
const instance = {
  type: "instance",
  id: "instance",
  name: "Card",
  width: 360,
  height: 257,
  componentId: "card",
  slotContent: { slot: [{ type: "rect", id: "old", name: "Existing", width: 20, height: 20 }] },
} as Node;
const item = (node: Node) => ({
  node,
  bounds: { x: 0, y: 0, width: 360, height: 257 },
  children: [],
  parent: null,
});
describe("authoring contracts", () => {
  it("finds slots on an instance without confusing them with ordinary children", () => {
    expect(slotTargets(item(instance), doc)).toMatchObject([
      {
        id: "slot",
        instance: "instance",
        component: "card",
        suggested: [{ id: "row", name: "Row", available: true }],
      },
    ]);
  });
  it("one-click insert appends an instance, preserves prior slot content and uses the injected id", () => {
    const slot = slotTargets(item(instance), doc)[0]!;
    const op = insertSlotOp(slot, "row", "new", doc, item(instance));
    expect(op).toMatchObject({
      type: "update",
      id: "instance",
      patch: {
        slotContent: {
          slot: [{ id: "old" }, { type: "instance", id: "new", componentId: "row", name: "Row" }],
        },
      },
    });
  });
  it("suggestions guide but do not restrict insertion", () => {
    const slot = slotTargets(item(instance), doc)[0]!;
    expect(insertSlotOp(slot, "card", "new", doc, item(instance))).toMatchObject({
      patch: { slotContent: { slot: expect.any(Array) } },
    });
  });
  it("refuses a removed component and cannot insert into a definition", () => {
    const slot = slotTargets(item(instance), doc)[0]!;
    expect(() => insertSlotOp(slot, "gone", "new", doc, item(instance))).toThrow(/unavailable/);
    expect(() =>
      insertSlotOp({ ...slot, instance: null }, "row", "new", doc, item(instance)),
    ).toThrow(/instance/);
  });
  it("accepts typed header controls and rejects invalid numeric, enum, color and boolean values", () => {
    expect(parseScriptInput({ name: "gap", kind: "number", min: 0, max: 24 }, "16")).toBe(16);
    expect(parseScriptInput({ name: "mode", kind: "enum", options: ["a", "b"] }, "a")).toBe("a");
    for (const [def, value] of [
      [{ name: "gap", kind: "number", min: 0 }, "-1"],
      [{ name: "gap", kind: "number" }, "Infinity"],
      [{ name: "mode", kind: "enum", options: ["a"] }, "b"],
      [{ name: "color", kind: "color" }, "red"],
      [{ name: "flag", kind: "boolean" }, "yes"],
    ] as const)
      expect(() => parseScriptInput(def as never, value)).toThrow();
  });
  it("never displays cached output or errors from another engine revision", () => {
    const node = {
      type: "script",
      id: "script",
      name: "Chart",
      src: "chart.js",
      inputs: {},
      width: 100,
      height: 100,
    } as Node;
    expect(
      scriptView(
        { revision: 2, entries: { script: { status: "ready", inputs: [] } } },
        1,
        "script",
        node,
      ).status,
    ).toBe("unavailable");
    expect(
      scriptView(
        {
          revision: 1,
          entries: {
            script: { status: "error", inputs: [], message: "bad", line: 18, excerpt: "18 bad()" },
          },
        },
        1,
        "script",
        node,
      ),
    ).toMatchObject({ status: "error", message: "bad", line: 18 });
  });
  it("empty and missing runtime states stay explicit", () => {
    expect(scriptView(undefined, 0, "s", { type: "script", src: "" } as Node).status).toBe("empty");
    expect(scriptView(undefined, 0, "s", { type: "script", src: "a.js" } as Node).status).toBe(
      "unavailable",
    );
  });
  it("guidelines save uses the loaded version, retains dirty text on conflict, and makes no document Ops", async () => {
    const requests: unknown[] = [];
    const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false });
    const c = createAuthoringController({
      engine,
      device: "device",
      nextNodeId: () => "new",
      host: {
        loadGuidelines: async () => ({
          name: "System",
          path: "system/guidelines.md",
          text: "# Rules",
          version: "v1",
          writable: true,
        }),
        saveGuidelines: async (request) => {
          requests.push(request);
          throw Error("Guidelines changed on disk");
        },
      },
    });
    await c.openGuidelines();
    c.editGuidelines("# New");
    await c.saveGuidelines();
    expect(requests).toEqual([{ path: "system/guidelines.md", text: "# New", version: "v1" }]);
    expect(c.state.guidelines?.text).toBe("# New");
    expect(c.state.error).toMatch(/changed on disk/);
    expect(engine.transactions).toHaveLength(0);
  });
  it("a late guidelines load cannot reopen a closed editor", async () => {
    let resolve!: (v: never) => void;
    const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false });
    const c = createAuthoringController({
      engine,
      device: "device",
      nextNodeId: () => "new",
      host: {
        loadGuidelines: () =>
          new Promise((r) => {
            resolve = r;
          }),
      },
    });
    const promise = c.openGuidelines();
    c.close();
    resolve({
      name: "System",
      path: "guidelines.md",
      text: "old",
      version: "1",
      writable: true,
    } as never);
    await promise;
    expect(c.state.dialog).toBeNull();
    expect(c.state.guidelines).toBeNull();
  });
  it("read-only prevents host writes and conversion", async () => {
    let saved = false;
    const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: true });
    const c = createAuthoringController({
      engine,
      device: "device",
      nextNodeId: () => "new",
      host: {
        saveGuidelines: async () => {
          saved = true;
          return { version: "2" };
        },
      },
    });
    await c.saveGuidelines();
    c.convertScript("script");
    expect(saved).toBe(false);
    expect(engine.transactions).toHaveLength(0);
  });
});

it("nested instance slots write an override of the selected nested instance", () => {
  const nested = { ...item(instance), instance: { id: "outer", path: "root/instance" } };
  const slot = slotTargets(nested, doc)[0]!;
  expect(insertSlotOp(slot, "row", "new", doc, nested)).toMatchObject({
    type: "replaceInInstance",
    id: "outer",
    path: "root/instance",
    node: { id: "instance", slotContent: { slot: [{ id: "old" }, { id: "new" }] } },
  });
});
it("image import cannot write stale results after the engine changes", async () => {
  const picture = {
    type: "frame",
    id: "picture",
    name: "Picture",
    width: 100,
    height: 100,
    fills: [{ type: "image", asset: "a".repeat(64) }],
  } as Node;
  const scene = { get: () => item(picture) } as never;
  const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false }, scene);
  let resolve!: (v: { asset: string }) => void;
  const c = createAuthoringController({
    engine,
    device: "device",
    nextNodeId: () => "new",
    host: {
      chooseImage: () =>
        new Promise((r) => {
          resolve = r;
        }),
    },
  });
  c.openImage("picture", "fill", 0);
  const pending = c.importImage("choose");
  engine.script({ document: doc, revision: 1, readOnly: false });
  engine.apply({
    origin: { actor: { kind: "person", device: "device" } },
    ops: [{ type: "update", id: "picture", patch: { name: "Changed" } }],
  });
  resolve({ asset: "b".repeat(64) });
  await pending;
  expect(engine.transactions).toHaveLength(1);
  expect(c.state.error).toMatch(/document changed/);
});
it("closing an image picker cancels its late import result", async () => {
  let resolve!: (v: { asset: string }) => void;
  const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false });
  const c = createAuthoringController({
    engine,
    device: "device",
    nextNodeId: () => "new",
    host: {
      chooseImage: () =>
        new Promise((r) => {
          resolve = r;
        }),
    },
  });
  c.openImage("picture", "fill", 0);
  const pending = c.importImage("choose");
  c.close();
  resolve({ asset: "b".repeat(64) });
  await pending;
  expect(engine.transactions).toHaveLength(0);
  expect(c.state.dialog).toBeNull();
});
it("stock search ignores older replies and close invalidates pending results", async () => {
  let first!: (v: { photos: never[]; total: number }) => void;
  let second!: (v: { photos: never[]; total: number }) => void;
  const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false });
  const c = createAuthoringController({
    engine,
    device: "device",
    nextNodeId: () => "new",
    host: {
      searchStock: (q) =>
        new Promise((r) => {
          if (q === "old") first = r;
          else second = r;
        }),
    },
  });
  const a = c.search("old"),
    b = c.search("new");
  second({ photos: [], total: 2 });
  await b;
  first({ photos: [], total: 1 });
  await a;
  expect(c.state.total).toBe(2);
  const pending = c.search("new");
  const before = c.state.total;
  c.close();
  second({ photos: [], total: 3 });
  await pending;
  expect(c.state.total).toBe(before);
});
it("image fit edits address the picker's target even after the selection changes", () => {
  const picture = {
    type: "frame",
    id: "picture",
    name: "Picture",
    width: 100,
    height: 100,
    fills: [{ type: "image", asset: "a".repeat(64) }],
  } as Node;
  const scene = { get: (id: string) => (id === "picture" ? item(picture) : undefined) } as never;
  const engine = new FakeEngineClient({ document: doc, revision: 0, readOnly: false }, scene);
  const c = createAuthoringController({
    engine,
    device: "device",
    nextNodeId: () => "new",
    host: {},
  });
  c.openImage("picture", "fill", 0);
  engine.script({ document: doc, revision: 1, readOnly: false });
  c.editImage((p) => ({ ...p, fit: "fit" }) as never);
  expect(engine.transactions[0]).toMatchObject({
    ops: [{ type: "update", id: "picture", patch: { fills: [{ fit: "fit" }] } }],
  });
});

it("color controls accept the core colour shorthand and normalize valid hex", () => {
  expect(parseScriptInput({ name: "bar", kind: "color" }, "#abc")).toBe("#AABBCC");
});
