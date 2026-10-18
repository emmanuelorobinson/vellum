// TEST DATA: variables, theme axes and synthetic scene nodes.
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { defaultMode, resolveVariable } from "../design/bindings";
import {
  addThemeAxis,
  addThemeValue,
  editVariable,
  frameTheme,
  themeColumns,
  variableCollections,
  variableEntry,
} from "./variables";

const doc = (): Document => ({
  meta: { formatVersion: 1, name: "test", createdWith: "test" },
  themes: { Mode: ["Light", "Dark"], Device: ["Phone", "Tablet"] },
  variables: {
    a: {
      type: "number",
      name: "space/4",
      values: [{ value: 16 }, { value: 24, when: { Device: "Tablet" } }],
    },
    b: { type: "number", name: "space/alias", values: [{ $alias: "a" }] },
  },
  pages: [],
});
describe("variable editor", () => {
  it("groups by the first name segment", () =>
    expect(variableCollections(doc())).toEqual(["space"]));
  it("groups combinations with the first axis varying fastest", () =>
    expect(themeColumns(doc()).map((c) => c.label)).toEqual([
      "Light / Phone",
      "Dark / Phone",
      "Light / Tablet",
      "Dark / Tablet",
    ]));
  it("has one default column without axes", () =>
    expect(themeColumns({ ...doc(), themes: {} })).toEqual([{ label: "Default", mode: {} }]));
  it("does not fabricate a partial table when axes overflow", () =>
    expect(() =>
      themeColumns({ ...doc(), themes: { A: Array.from({ length: 129 }, (_, i) => String(i)) } }),
    ).toThrow("128"));
  it("reads an alias without breaking its binding", () =>
    expect(variableEntry(doc(), "b", defaultMode(doc()))).toEqual({ $alias: "a" }));
  it("adds a specific value after fallback entries; other modes stay intact", () => {
    const d = doc();
    const op = editVariable(d, "a", { Mode: "Dark", Device: "Tablet" }, "32");
    expect(op.type).toBe("setVariables");
    if (op.type !== "setVariables") return;
    const next = { ...d, variables: { ...d.variables, ...op.set } };
    expect(resolveVariable(next, "a", { Mode: "Dark", Device: "Tablet" })).toBe(32);
    expect(resolveVariable(next, "a", { Mode: "Light", Device: "Tablet" })).toBe(24);
    expect(d.variables?.a?.values).toHaveLength(2);
  });
  it.each(["NaN", "Infinity", "", "2x"])("refuses invalid numbers %s", (v) =>
    expect(() => editVariable(doc(), "a", {}, v)).toThrow(),
  );
  it("refuses aliases of the wrong type and missing aliases", () => {
    const d = doc();
    d.variables = {
      ...d.variables,
      c: { type: "string", name: "type/family", values: [{ value: "Inter" }] },
    };
    expect(() => editVariable(d, "a", {}, "$c")).toThrow();
    expect(() => editVariable(d, "a", {}, "$missing")).toThrow();
  });
  it("refuses cycles including conditional cycles", () =>
    expect(() => editVariable(doc(), "a", { Mode: "Dark" }, "$b")).toThrow("cycle"));
  it("accepts a same-type alias", () =>
    expect(editVariable(doc(), "b", {}, "$a")).toMatchObject({
      set: { b: { values: [{ $alias: "a" }] } },
    }));
  it("parses booleans strictly and keeps strings as strings", () => {
    const d = doc();
    d.variables = {
      yes: { type: "boolean", name: "state/yes", values: [{ value: true }] },
      text: { type: "string", name: "type/name", values: [{ value: "" }] },
    };
    expect(editVariable(d, "yes", {}, "false")).toMatchObject({
      set: { yes: { values: [{ value: false }] } },
    });
    expect(() => editVariable(d, "yes", {}, "False")).toThrow();
    expect(editVariable(d, "text", {}, "#FFFFFF")).toMatchObject({
      set: { text: { values: [{ value: "#FFFFFF" }] } },
    });
  });
  it("adds unique axes and values without replacing existing ones", () => {
    expect(addThemeAxis(doc(), "Brand", "A")).toEqual({ ...doc().themes, Brand: ["A"] });
    expect(addThemeValue(doc(), "Mode", "Dim").Mode).toEqual(["Light", "Dark", "Dim"]);
    expect(() => addThemeAxis(doc(), "Mode", "X")).toThrow();
    expect(() => addThemeValue(doc(), "Mode", "Light")).toThrow();
  });
  it("inherits valid frame axes, ignores unknown axes and allows a child override", () => {
    const d = doc();
    const nodes: Record<string, { node: Node; parent: string | null }> = {
      parent: {
        node: {
          id: "parent",
          type: "frame",
          name: "Parent",
          theme: { Mode: "Dark", Device: "Phone", Unknown: "x" },
        } as unknown as Node,
        parent: null,
      },
      child: {
        node: {
          id: "child",
          type: "frame",
          name: "Child",
          theme: { Device: "Tablet", Mode: "missing" },
        } as unknown as Node,
        parent: "parent",
      },
    };
    expect(frameTheme(d, "child", (id) => nodes[id])).toEqual({ Mode: "Dark", Device: "Tablet" });
    expect(frameTheme(d, null, (id) => nodes[id])).toEqual(defaultMode(d));
  });
});

it("presents Mode first even when the codec orders theme-map keys alphabetically", () => {
  const d = doc();
  d.themes = { Device: ["Phone", "Tablet"], Mode: ["Light", "Dark"] };
  expect(themeColumns(d).map((c) => c.label)).toEqual([
    "Light / Phone",
    "Dark / Phone",
    "Light / Tablet",
    "Dark / Tablet",
  ]);
});
