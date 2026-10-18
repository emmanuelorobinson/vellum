// TEST DATA: a scripted engine over a small Juniper document; no renderer, DOM or document store.
import { type EngineSnapshot, FakeEngineClient, type ToolScene } from "@vellum/engine-client";
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { createEditorSession } from "../session";
import type { SceneItem } from "../tool-types";
import { createDesignController } from "./controller";
import { setOpacity, setPosition } from "./patches";

const nodes = (): Node[] =>
  [
    {
      type: "frame",
      id: "step2",
      name: "Step 2",
      width: 240,
      height: 440,
      position: { type: "absolute", x: 320, y: 120 },
      layout: "column",
      children: [
        { type: "rect", id: "card", name: "Card", width: 100, height: 40 },
        {
          type: "text",
          id: "title",
          name: "Title",
          width: 100,
          height: 20,
          content: [{ text: "Hi", font: "Inter", size: 16 }],
        },
      ],
    },
    {
      type: "rect",
      id: "free",
      name: "Free",
      width: 10,
      height: 10,
      position: { type: "absolute", x: 5, y: 5 },
    },
  ] as unknown as Node[];
const doc = (): Document =>
  ({
    meta: { formatVersion: 1, name: "t", createdWith: "test" },
    themes: { Mode: ["Light", "Dark"] },
    variables: {
      c1: {
        type: "color",
        name: "color/surface",
        values: [{ value: "#FBF9F3" }, { value: "#17181B", when: { Mode: "Dark" } }],
      },
    },
    pages: [{ id: "page1", name: "Page", children: nodes() }],
  }) as unknown as Document;

function sceneOf(): ToolScene {
  const items = new Map<string, SceneItem>();
  const walk = (list: Node[], parent: string | null) => {
    for (const n of list) {
      const kids = (n as unknown as { children?: Node[] }).children ?? [];
      items.set(n.id, {
        node: n,
        bounds: { x: 0, y: 0, width: 10, height: 10 },
        children: kids.map((k) => k.id),
        parent,
      });
      walk(kids, n.id);
    }
  };
  walk(nodes(), null);
  return {
    get: (id) => items.get(id),
    roots: () => ["step2", "free"],
    hitPoint: (pt) => (pt.x === 1 && pt.y === 1 ? "title" : null),
    hitRect: () => [],
    snap: () => ({ offset: { x: 0, y: 0 }, guides: [] }),
    toParent: (_a, p) => p,
    toLocal: (_a, p) => p,
    path: () => null,
  };
}
function fixture(readOnly = false) {
  const snap = (revision: number): EngineSnapshot => ({ revision, document: doc(), readOnly });
  const engine = new FakeEngineClient(snap(0), sceneOf());
  const session = createEditorSession({
    engine,
    device: "dev1",
    page: "page1",
    nextNodeId: () => "n1",
    createNode: () => nodes()[1] as Node,
    viewportSize: () => ({ width: 800, height: 600 }),
  });
  const design = createDesignController({
    session,
    engine,
    defaults: { fill: "#D9D9D9", stroke: "#1F1F1F", shadow: "#00000040" },
    text: { fallback: "Inter" },
  });
  return {
    engine,
    session,
    design,
    reply: () => engine.script(snap(engine.snapshot().revision + 1)),
  };
}

describe("design controller: reading", () => {
  it("has no targets and an empty model without a selection", () => {
    const { design } = fixture();
    expect(design.targets()).toEqual([]);
    expect(design.model().sections).toEqual([]);
  });
  it("builds the model of the selected layers from the engine's snapshot", () => {
    const { design, session } = fixture();
    session.select(["step2"]);
    const m = design.model();
    expect(m.kinds).toEqual(["frame"]);
    expect(m.position.x.value).toEqual({ kind: "value", value: 320 });
  });
  it("knows a layer is placed by its parent's layout", () => {
    const { design, session } = fixture();
    session.select(["card"]);
    expect(design.targets()[0]?.inLayout).toBe(true);
    session.select(["free"]);
    expect(design.targets()[0]?.inLayout).toBe(false);
  });
  it("is read-only when the file is", () => {
    const { design, session } = fixture(true);
    session.select(["step2"]);
    expect(design.model().readOnly).toBe(true);
  });
  it("is editing while a text layer is open for typing, and then shows only Text, Fill and Content", () => {
    const { design, session } = fixture();
    expect(design.editing()).toBe(false);
    session.dispatch({ type: "doubleClick", point: { x: 1, y: 1 } });
    expect(design.editing()).toBe(true);
    expect(design.model().sections).toEqual(["text", "fill", "content"]);
    session.dispatch({ type: "cancel" });
    expect(design.editing()).toBe(false);
  });
  it("resolves a colour variable under the mode it is asked for", () => {
    const { design } = fixture();
    expect(design.colour({ $var: "c1" } as never)).toBe("#FBF9F3");
    expect(design.colour("#123456")).toBe("#123456");
    design.setMode({ Mode: "Dark" });
    expect(design.colour({ $var: "c1" } as never)).toBe("#17181B");
  });
});

describe("design controller: writing", () => {
  it("sends one Person transaction for a change to the whole selection", () => {
    const { design, session, engine, reply } = fixture();
    session.select(["step2", "free"]);
    reply();
    expect(design.edit(setOpacity(0.5))).toMatchObject({ status: "applied" });
    expect(engine.transactions).toHaveLength(1);
    expect(engine.transactions[0]?.origin.actor).toEqual({ kind: "person", device: "dev1" });
    expect(engine.transactions[0]?.ops).toEqual([
      { type: "update", id: "step2", patch: { opacity: 0.5 } },
      { type: "update", id: "free", patch: { opacity: 0.5 } },
    ]);
  });
  it("skips the layers an edit does not apply to, and sends nothing when none do", () => {
    const { design, session, engine, reply } = fixture();
    session.select(["card"]);
    reply();
    expect(design.edit(setPosition("x", 3))).toBeNull();
    expect(engine.transactions).toHaveLength(0);
  });
  it("refuses to write in a read-only file and says so", () => {
    const { design, session, engine } = fixture(true);
    session.select(["step2"]);
    expect(design.edit(setOpacity(0.5))).toMatchObject({ status: "rejected", reason: "read-only" });
    expect(session.notice).toEqual({ kind: "rejected", reason: "read-only" });
    expect(engine.transactions).toHaveLength(0);
  });
  it("reports a refusal from the engine", () => {
    const { design, session } = fixture();
    session.select(["step2"]);
    expect(design.edit(setOpacity(0.5))).toMatchObject({ status: "rejected" });
    expect(session.notice?.kind).toBe("rejected");
  });
  it("sends core requests as they are, for the component actions", () => {
    const { design, session, engine, reply } = fixture();
    session.select(["step2"]);
    reply();
    expect(design.ops([{ type: "detach", id: "step2" }])).toMatchObject({ status: "applied" });
    expect(engine.transactions[0]?.ops).toEqual([{ type: "detach", id: "step2" }]);
    expect(design.ops([])).toBeNull();
  });
  it("changes a part of an instance through an override", () => {
    const { design, session, engine, reply } = fixture();
    session.select(["step2"]);
    const item = session.sceneItem("step2") as SceneItem;
    // Make the scene say this layer is part of an instance.
    (engine.scene as ToolScene).get = (id) =>
      id === "step2" ? { ...item, instance: { id: "inst", path: "inst/step2" } } : undefined;
    reply();
    design.edit(setOpacity(0.5));
    expect(engine.transactions[0]?.ops[0]).toMatchObject({
      type: "replaceInInstance",
      id: "inst",
      path: "inst/step2",
      node: { id: "step2", opacity: 0.5 },
    });
  });
});

describe("design controller: text being typed", () => {
  it("reads the draft, not the saved layer, while the text is open for typing", () => {
    const { design, session } = fixture();
    session.dispatch({ type: "doubleClick", point: { x: 1, y: 1 } });
    session.dispatch({ type: "content", content: [{ text: "Typed", font: "Inter", size: 40 }] });
    const text = design.model().text;
    expect(text?.size).toEqual({ kind: "value", value: 40 });
    expect(text?.content).toBe("Typed");
    session.dispatch({ type: "cancel" });
    expect(design.model().text?.size).toEqual({ kind: "value", value: 16 });
  });
});

describe("design controller: single", () => {
  it("is the one selected layer's target, and nothing for none or several", () => {
    const { design, session } = fixture();
    expect(design.single()).toBeNull();
    session.select(["step2"]);
    expect(design.single()?.item.node.id).toBe("step2");
    session.select(["step2", "free"]);
    expect(design.single()).toBeNull();
  });
});
