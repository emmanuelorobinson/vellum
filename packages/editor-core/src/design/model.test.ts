// TEST DATA: the Step 2 frame of design screen C13 (Juniper onboarding).
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { type DesignInput, type DesignTarget, readDesign, sectionsFor } from "./model";

const doc = {
  meta: { formatVersion: 1, name: "juniper.vellum", createdWith: "test" },
  themes: { Mode: ["Light", "Dark"] },
  variables: {
    v1: { type: "number", name: "space/4", values: [{ value: 16 }] },
    v2: {
      type: "color",
      name: "color/surface",
      values: [{ value: "#FBF9F3" }, { value: "#17181B", when: { Mode: "Dark" } }],
    },
  },
  pages: [],
} as unknown as Document;

const frame = (extra: Record<string, unknown> = {}, id = "step2"): DesignTarget => ({
  item: {
    node: {
      type: "frame",
      id,
      name: "Step 2",
      width: "hug",
      height: "hug",
      position: { type: "absolute", x: 320, y: 120 },
      layout: "column",
      gap: { $var: "v1" },
      padding: 24,
      cornerRadius: 8,
      fills: [{ type: "solid", color: { $var: "v2" } }],
      ...extra,
    } as unknown as Node,
    bounds: { x: 320, y: 120, width: 240, height: 440 },
    children: [],
    parent: null,
  },
  inLayout: false,
});
const input = (targets: DesignTarget[], extra: Partial<DesignInput> = {}): DesignInput => ({
  targets,
  document: doc,
  readOnly: false,
  text: { fallback: "Inter" },
  ...extra,
});

describe("sectionsFor", () => {
  const kinds = (...k: string[]) => sectionsFor(k as never);
  it("gives a frame the sections of design screen C13, Notes for agents first", () => {
    expect(kinds("frame")).toEqual([
      "notes",
      "position",
      "layout",
      "appearance",
      "fill",
      "stroke",
      "effects",
      "export",
    ]);
  });
  it("puts what is particular to the layer first: Text for text, Instance for an instance", () => {
    expect(kinds("text")[0]).toBe("text");
    expect(kinds("text")).not.toContain("notes");
    expect(kinds("instance")[0]).toBe("component");
    expect(kinds("frame", "text")).not.toContain("text");
    expect(kinds("frame", "instance")).not.toContain("component");
  });
  it("puts Component first for a main component, which is a frame without notes", () => {
    expect(sectionsFor(["frame"], { main: true })[0]).toBe("component");
    expect(sectionsFor(["frame"], { main: true })).not.toContain("notes");
  });
  it("shows only Text, Fill and Content while text is being edited (C14)", () => {
    expect(sectionsFor(["text"], { editing: true })).toEqual(["text", "fill", "content"]);
  });
  it("gives a group no fill or stroke, a line no fill", () => {
    expect(kinds("group")).not.toContain("fill");
    expect(kinds("group")).not.toContain("stroke");
    expect(kinds("line")).not.toContain("fill");
    expect(kinds("line")).toContain("stroke");
  });
  it("gives the agent-facing layers only what they can have", () => {
    expect(kinds("note")).toEqual(["position", "layout"]);
    expect(kinds("prompt")).toEqual(["position", "layout"]);
  });
  it("is empty for no selection", () => {
    expect(kinds()).toEqual([]);
  });
});

describe("readDesign: position", () => {
  it("reads X, Y and rotation", () => {
    const m = readDesign(input([frame({ rotation: 15 })]));
    expect(m.position.x).toMatchObject({ value: { kind: "value", value: 320 }, editable: true });
    expect(m.position.y).toMatchObject({ value: { kind: "value", value: 120 } });
    expect(m.position.rotation.value).toEqual({ kind: "value", value: 15 });
  });
  it("reads rotation 0 when there is none", () => {
    expect(readDesign(input([frame()])).position.rotation.value).toEqual({
      kind: "value",
      value: 0,
      isDefault: true,
    });
  });
  it("cannot move a layer that its parent's layout places, and says why", () => {
    const m = readDesign(input([{ ...frame({ position: { type: "auto" } }), inLayout: true }]));
    expect(m.position.x).toMatchObject({
      editable: false,
      reason: "Placed by its parent's layout",
    });
  });
  it("shows the origin for a layer on a page with no position, and lets it be set", () => {
    const m = readDesign(input([frame({ position: undefined })]));
    expect(m.position.x).toMatchObject({ value: { kind: "value", value: 0 }, editable: true });
  });
  it("is mixed across layers", () => {
    const b = frame({ position: { type: "absolute", x: 10, y: 120 } }, "b");
    const m = readDesign(input([frame(), b]));
    expect(m.position.x.value).toEqual({ kind: "mixed" });
    expect(m.position.y.value).toMatchObject({ kind: "value", value: 120 });
  });
  it("cannot be edited in a read-only file", () => {
    expect(readDesign(input([frame()], { readOnly: true })).position.x).toMatchObject({
      editable: false,
      reason: "This file is read-only",
    });
  });
});

describe("readDesign: size", () => {
  it("reads hug, fill and fixed per axis, showing the size the layer has now", () => {
    const m = readDesign(input([frame({ width: "hug", height: 440 })]));
    expect(m.size.width).toEqual({ mode: "hug", value: 240, mixed: false });
    expect(m.size.height).toEqual({ mode: "fixed", value: 440, mixed: false });
    expect(readDesign(input([frame({ width: "fill" })])).size.width.mode).toBe("fill");
  });
  it("treats a size with a fallback as hug or fill", () => {
    expect(readDesign(input([frame({ width: { hug: 100 } })])).size.width.mode).toBe("hug");
    expect(readDesign(input([frame({ width: { fill: 100 } })])).size.width.mode).toBe("fill");
  });
  it("is mixed when layers differ", () => {
    const m = readDesign(input([frame(), frame({ width: 100 }, "b")]));
    expect(m.size.width.mixed).toBe(true);
  });
  it("shows a variable-bound size as fixed and bound", () => {
    const m = readDesign(input([frame({ width: { $var: "v1" } })]));
    expect(m.size.width).toMatchObject({
      mode: "fixed",
      bound: { id: "v1", name: "space/4", value: 16 },
    });
  });
});

describe("readDesign: layout", () => {
  it("reads the layout of a frame as the design shows it", () => {
    const l = readDesign(input([frame()])).layout;
    expect(l?.mode).toEqual({ kind: "value", value: "column" });
    expect(l?.gap).toEqual({
      kind: "bound",
      variable: { id: "v1", name: "space/4", value: 16 },
    });
    expect(l?.padding.linked).toBe(true);
    expect(l?.padding.all).toEqual({ kind: "value", value: 24 });
  });
  it("is Free when the frame has no layout", () => {
    expect(readDesign(input([frame({ layout: undefined })])).layout?.mode).toEqual({
      kind: "value",
      value: "none",
      isDefault: true,
    });
  });
  it("has no layout for a layer that is not a frame", () => {
    expect(readDesign(input([frame({ type: "rect" })])).layout).toBeNull();
    expect(readDesign(input([frame(), frame({ type: "rect" }, "r")])).layout).toBeNull();
  });
  it("reads padding written as two values, or four, as unlinked sides", () => {
    const two = readDesign(input([frame({ padding: [8, 16] })])).layout?.padding;
    expect(two?.linked).toBe(false);
    expect(two?.sides.map((s) => (s.kind === "value" ? s.value : null))).toEqual([8, 16, 8, 16]);
    const four = readDesign(input([frame({ padding: [1, 2, 3, 4] })])).layout?.padding;
    expect(four?.sides.map((s) => (s.kind === "value" ? s.value : null))).toEqual([1, 2, 3, 4]);
  });
  it("calls four equal sides linked", () => {
    expect(readDesign(input([frame({ padding: [4, 4, 4, 4] })])).layout?.padding.linked).toBe(true);
  });
  it("shows padding that is a variable as bound", () => {
    expect(
      readDesign(input([frame({ padding: { $var: "v1" } })])).layout?.padding.all,
    ).toMatchObject({
      kind: "bound",
    });
  });
  it("reads alignment and distribution, with their defaults", () => {
    const l = readDesign(input([frame({ align: "center" })])).layout;
    expect(l?.align).toEqual({ kind: "value", value: "center" });
    expect(l?.justify).toEqual({ kind: "value", value: "start", isDefault: true });
  });
  it("offers grid rows and columns only for a grid", () => {
    expect(readDesign(input([frame()])).layout?.grid).toBeNull();
    expect(readDesign(input([frame({ layout: "grid", columns: 3 })])).layout?.grid).toEqual({
      columns: { kind: "value", value: 3 },
      rows: { kind: "value", value: null, isDefault: true },
    });
  });
});

describe("readDesign: appearance", () => {
  it("reads opacity, radius and clip", () => {
    const a = readDesign(input([frame({ opacity: 0.5, clip: true })])).appearance;
    expect(a.opacity).toEqual({ kind: "value", value: 0.5 });
    expect(a.radius).toEqual({ kind: "value", value: 8 });
    expect(a.clip).toEqual({ kind: "value", value: true });
    expect(a.blend).toEqual({ kind: "value", value: "normal", isDefault: true });
  });
  it("shows four different corner radii as mixed", () => {
    expect(readDesign(input([frame({ cornerRadius: [4, 8, 4, 8] })])).appearance.radius).toEqual({
      kind: "mixed",
    });
    expect(readDesign(input([frame({ cornerRadius: [8, 8, 8, 8] })])).appearance.radius).toEqual({
      kind: "value",
      value: 8,
    });
  });
  it("has no radius or clip where the layer has none", () => {
    const a = readDesign(input([frame({ type: "ellipse" })])).appearance;
    expect(a.radius).toBeNull();
    expect(a.clip).toBeNull();
  });
});

describe("readDesign: fill", () => {
  it("lists each paint as a row: its kind, what to draw, and what it is bound to", () => {
    const f = readDesign(input([frame()])).fills;
    if (f.kind !== "list") throw Error("list");
    expect(f.rows).toEqual([
      {
        index: 0,
        kind: "solid",
        label: "Surface",
        swatch: "#FBF9F3",
        text: "#FBF9F3",
        opacity: 1,
        visible: true,
        bound: { id: "v2", name: "color/surface", value: "#FBF9F3" },
      },
    ]);
  });
  it("resolves a bound colour under the mode of the layer", () => {
    const f = readDesign(input([frame()], { mode: { Mode: "Dark" } })).fills;
    if (f.kind !== "list") throw Error("list");
    expect(f.rows[0]?.text).toBe("#17181B");
  });
  it("calls an unbound solid Color, and shows hex and opacity", () => {
    const f = readDesign(input([frame({ fills: [{ type: "solid", color: "#234D3C80" }] })])).fills;
    if (f.kind !== "list") throw Error("list");
    expect(f.rows[0]).toMatchObject({ label: "Color", text: "#234D3C80", bound: null });
    expect(f.rows[0]?.opacity).toBeCloseTo(0.5, 2);
  });
  it("is empty without paints, and mixed when layers differ", () => {
    expect(readDesign(input([frame({ fills: undefined })])).fills).toEqual({
      kind: "list",
      rows: [],
    });
    expect(
      readDesign(input([frame(), frame({ fills: [{ type: "solid", color: "#000000" }] }, "b")]))
        .fills,
    ).toEqual({ kind: "mixed" });
  });
  it("marks a hidden paint", () => {
    const f = readDesign(
      input([frame({ fills: [{ type: "solid", color: "#000000", visible: false }] })]),
    ).fills;
    if (f.kind !== "list") throw Error("list");
    expect(f.rows[0]?.visible).toBe(false);
  });
  it("describes a gradient, a mesh and an image by their kind", () => {
    const rows = (paints: unknown[]) => {
      const f = readDesign(input([frame({ fills: paints })])).fills;
      return f.kind === "list" ? f.rows : [];
    };
    const [g, m, i] = rows([
      {
        type: "gradient",
        kind: "linear",
        stops: [
          { color: "#000000", position: 0 },
          { color: "#FFFFFF", position: 1 },
        ],
      },
      {
        type: "mesh",
        columns: 2,
        rows: 2,
        points: [0, 1, 2, 3].map((n) => ({ x: n % 2, y: n >> 1, color: "#123456" })),
      },
      { type: "image", asset: "a1" },
    ]);
    expect([g?.kind, m?.kind, i?.kind]).toEqual(["linear", "mesh", "image"]);
    expect(g?.label).toBe("Linear gradient");
    expect(i?.swatch).toBeNull();
  });
});

describe("readDesign: stroke and effects", () => {
  it("is none without a stroke", () => {
    expect(readDesign(input([frame()])).stroke).toEqual({ kind: "none" });
  });
  it("reads a stroke's settings, with defaults for what the file leaves out", () => {
    const s = readDesign(
      input([
        frame({
          stroke: {
            paints: [{ type: "solid", color: "#234D3C" }],
            width: [1, 1, 2, 1],
            dash: [4, 4],
            cap: "round",
          },
        }),
      ]),
    ).stroke;
    if (s.kind !== "stroke") throw Error("stroke");
    expect(s.summary).toBe("Top 1 · Right 1 · Bottom 2 · Left 1");
    expect(s.align).toBe("center");
    expect(s.cap).toBe("round");
    expect(s.join).toBe("miter");
    expect(s.dash).toBe("4, 4");
    expect(s.rows).toHaveLength(1);
  });
  it("lists effects as rows", () => {
    const e = readDesign(
      input([
        frame({
          effects: [
            { type: "drop-shadow", x: 0, y: 4, blur: 16, color: "#00000040" },
            { type: "layer-blur", radius: 8, visible: false },
          ],
        }),
      ]),
    ).effects;
    if (e.kind !== "list") throw Error("list");
    expect(e.rows.map((r) => [r.label, r.summary, r.visible])).toEqual([
      ["Drop shadow", "0, 4 · blur 16", true],
      ["Layer blur", "8 px · hidden", false],
    ]);
  });
  it("is empty without effects", () => {
    expect(readDesign(input([frame()])).effects).toEqual({ kind: "list", rows: [] });
  });
});

describe("readDesign: notes and the rest", () => {
  it("reads the notes for agents, and nothing when there are none", () => {
    expect(readDesign(input([frame()])).notes).toEqual({
      kind: "value",
      value: "",
      isDefault: true,
    });
    expect(readDesign(input([frame({ agentNotes: "Copy under 10 words." })])).notes).toEqual({
      kind: "value",
      value: "Copy under 10 words.",
    });
  });
  it("counts the layers and names the kinds", () => {
    const m = readDesign(input([frame(), frame({ type: "rect" }, "r")]));
    expect(m.count).toBe(2);
    expect(m.kinds).toEqual(["frame", "rect"]);
  });
  it("has the text model only for text", () => {
    expect(readDesign(input([frame()])).text).toBeNull();
  });
  it("has the component model for an instance", () => {
    const i = frame({ type: "instance", componentId: "c1" });
    expect(readDesign(input([i])).component).toMatchObject({ kind: "instance" });
  });
});
