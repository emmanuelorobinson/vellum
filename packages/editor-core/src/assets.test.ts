import type { Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { type AssetsInput, assetSections, instanceTransaction } from "./assets";

const input = (over: Partial<AssetsInput> = {}): AssetsInput => ({
  components: {
    c1: { name: "Button / Primary" },
    c2: { name: "HabitRow" },
    c3: { name: "Preference toggle" },
  },
  libraries: [{ alias: "Juniper Core", state: "synced", components: 8 }],
  iconSets: [
    { id: "lucide", label: "Lucide" },
    { id: "material-symbols", label: "Material Symbols" },
    { id: "phosphor", label: "Phosphor" },
    { id: "feather", label: "Feather" },
  ],
  projectIconFolder: { path: "design/icons" },
  visualStyles: [
    { id: "minimal", name: "Minimal" },
    { id: "editorial", name: "Editorial" },
    { id: "technical", name: "Technical" },
  ],
  designSystems: [
    { id: "starter-minimal", name: "Starter Minimal" },
    { id: "starter-editorial", name: "Starter Editorial" },
  ],
  query: "",
  ...over,
});
const ids = (s: ReturnType<typeof assetSections>) => s.map((x) => x.id);

describe("asset sections", () => {
  it("lists the five sections in the design's order", () => {
    const s = assetSections(input());
    expect(ids(s)).toEqual(["document", "libraries", "icons", "styles", "systems"]);
    expect(s.map((x) => x.heading)).toEqual([
      "This document",
      "Libraries",
      "Icons",
      "Styles",
      "Starter design systems",
    ]);
  });
  it("lists document components by name, ready to drag", () => {
    const doc = assetSections(input())[0];
    expect(doc?.items.map((i) => [i.label, i.icon, i.payload])).toEqual([
      ["Button / Primary", "component", { kind: "component", componentId: "c1", library: null }],
      ["HabitRow", "component", { kind: "component", componentId: "c2", library: null }],
      ["Preference toggle", "component", { kind: "component", componentId: "c3", library: null }],
    ]);
  });
  it("shows a library with its state and component count, and a missing one as missing", () => {
    const lib = assetSections(
      input({
        libraries: [
          { alias: "Juniper Core", state: "synced", components: 8 },
          { alias: "Old kit", state: "missing", components: 3 },
        ],
      }),
    )[1];
    expect(lib?.items.map((i) => [i.label, i.detail, i.status])).toEqual([
      ["Juniper Core", "Synced · 8 components", "ok"],
      ["Old kit", "Missing · 3 components", "missing"],
    ]);
    expect(
      assetSections(input({ libraries: [{ alias: "a", state: "synced", components: 1 }] }))[1]
        ?.items[0]?.detail,
    ).toBe("Synced · 1 component");
  });
  it("offers the icon sets and the project's own SVG folder", () => {
    const icons = assetSections(input())[2];
    expect(icons?.items.map((i) => i.label)).toEqual([
      "Lucide · Material Symbols · Phosphor · Feather",
      "Your SVG folder",
    ]);
    expect(icons?.items[1]?.action).toBe("browse");
    expect(assetSections(input({ projectIconFolder: null }))[2]?.items).toHaveLength(1);
  });
  it("offers each starter design system with Add", () => {
    const s = assetSections(input())[4];
    expect(s?.items.map((i) => [i.label, i.action])).toEqual([
      ["Starter Minimal", "add"],
      ["Starter Editorial", "add"],
    ]);
  });
  it("omits a section with nothing in it and keeps the always-present ones", () => {
    const s = assetSections(input({ components: {}, libraries: [] }));
    expect(ids(s)).toEqual(["icons", "styles", "systems"]);
  });
});

describe("asset search", () => {
  it("matches every word against the label and the detail, ignoring case", () => {
    const s = assetSections(input({ query: "primary BUTTON" }));
    expect(ids(s)).toEqual(["document"]);
    expect(s[0]?.items.map((i) => i.label)).toEqual(["Button / Primary"]);
  });
  it("finds a library by its state and an icon set by any of its names", () => {
    expect(ids(assetSections(input({ query: "synced" })))).toEqual(["libraries"]);
    expect(ids(assetSections(input({ query: "phosphor" })))).toEqual(["icons"]);
  });
  it("finds a visual style inside the combined Styles row", () => {
    const s = assetSections(input({ query: "editorial" }));
    expect(ids(s)).toEqual(["styles", "systems"]);
  });
  it("returns nothing for a query nothing matches", () => {
    expect(assetSections(input({ query: "zzz" }))).toEqual([]);
  });
  it("treats blank space as no query", () => {
    expect(assetSections(input({ query: "   " }))).toHaveLength(5);
  });
});

describe("inserting a component", () => {
  const root = {
    type: "frame",
    id: "root",
    name: "Button",
    width: 120,
    height: { fill: 40 },
  } as unknown as Node;
  it("inserts an instance at the drop point with the component's size", () => {
    const tx = instanceTransaction({
      device: "d1",
      componentId: "c1",
      library: null,
      name: "Button / Primary",
      root,
      id: "n1",
      at: { x: 300, y: 520 },
      to: { parent: { type: "page", id: "p1" }, index: 3 },
    });
    expect(tx.origin).toEqual({ actor: { kind: "person", device: "d1" } });
    expect(tx.ops).toEqual([
      {
        type: "insert",
        to: { parent: { type: "page", id: "p1" }, index: 3 },
        node: {
          type: "instance",
          id: "n1",
          name: "Button / Primary",
          componentId: "c1",
          width: 120,
          height: "hug",
          position: { type: "absolute", x: 300, y: 520 },
        },
      },
    ]);
  });
  it("names the library for a library component", () => {
    const tx = instanceTransaction({
      device: "d1",
      componentId: "c1",
      library: "Juniper Core",
      name: "Card",
      root,
      id: "n1",
      at: { x: 0, y: 0 },
      to: { parent: { type: "page", id: "p1" }, index: 0 },
    });
    const op = tx.ops[0];
    expect(op?.type === "insert" && (op.node as { library?: string }).library).toBe("Juniper Core");
  });
});
