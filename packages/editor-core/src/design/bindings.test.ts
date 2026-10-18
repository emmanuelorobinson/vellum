// TEST DATA: a tiny document with a Mode axis, a number variable, an alias and a colour.
import type { Document } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import {
  defaultMode,
  describeValue,
  isVarRef,
  listVariables,
  readValue,
  resolveVariable,
} from "./bindings";

const doc = {
  meta: { formatVersion: 1, name: "t", createdWith: "t" },
  themes: { Mode: ["Light", "Dark"] },
  variables: {
    v1: { type: "number", name: "space/4", values: [{ value: 16 }] },
    v2: { type: "number", name: "space/gap", values: [{ $alias: "v1" }] },
    v3: {
      type: "color",
      name: "color/surface",
      values: [{ value: "#FBF9F3" }, { value: "#17181B", when: { Mode: "Dark" } }],
    },
    v4: { type: "color", name: "color/loop-a", values: [{ $alias: "v5" }] },
    v5: { type: "color", name: "color/loop-b", values: [{ $alias: "v4" }] },
    v6: { type: "boolean", name: "flag/show", values: [{ value: true }] },
  },
  pages: [],
} as unknown as Document;

describe("defaultMode", () => {
  it("is the first value of each axis", () => {
    expect(defaultMode(doc)).toEqual({ Mode: "Light" });
    expect(defaultMode(null)).toEqual({});
  });
});

describe("resolveVariable", () => {
  it("returns the literal", () => {
    expect(resolveVariable(doc, "v1", {})).toBe(16);
  });
  it("lets the last entry whose condition holds win", () => {
    expect(resolveVariable(doc, "v3", { Mode: "Light" })).toBe("#FBF9F3");
    expect(resolveVariable(doc, "v3", { Mode: "Dark" })).toBe("#17181B");
  });
  it("follows an alias", () => {
    expect(resolveVariable(doc, "v2", {})).toBe(16);
  });
  it("gives up on a cycle and on a missing variable instead of looping", () => {
    expect(resolveVariable(doc, "v4", {})).toBeUndefined();
    expect(resolveVariable(doc, "nope", {})).toBeUndefined();
  });
});

describe("isVarRef", () => {
  it("recognises only { $var }", () => {
    expect(isVarRef({ $var: "v1" })).toBe(true);
    expect(isVarRef({ $var: 3 })).toBe(false);
    expect(isVarRef(16)).toBe(false);
    expect(isVarRef(null)).toBe(false);
    expect(isVarRef({ $var: "v1", extra: 1 })).toBe(false);
  });
});

describe("listVariables", () => {
  it("lists the variables of one type, by name, with the value they have now", () => {
    expect(listVariables(doc, "number", {})).toEqual([
      { id: "v1", name: "space/4", type: "number", value: 16 },
      { id: "v2", name: "space/gap", type: "number", value: 16 },
    ]);
    expect(listVariables(doc, "color", { Mode: "Dark" }).map((v) => [v.name, v.value])).toEqual([
      ["color/loop-a", undefined],
      ["color/loop-b", undefined],
      ["color/surface", "#17181B"],
    ]);
  });
  it("is empty without a document", () => {
    expect(listVariables(null, "number", {})).toEqual([]);
  });
});

describe("readValue", () => {
  const read = (raws: unknown[], fallback = 0) => readValue(raws as never, doc, {}, fallback);
  it("shows a literal", () => {
    expect(read([8, 8])).toEqual({ kind: "value", value: 8 });
  });
  it("shows the default when nothing is set", () => {
    expect(read([undefined], 1)).toEqual({ kind: "value", value: 1, isDefault: true });
  });
  it("shows a binding with its name and what it resolves to", () => {
    expect(read([{ $var: "v1" }])).toEqual({
      kind: "bound",
      variable: { id: "v1", name: "space/4", value: 16 },
    });
  });
  it("says so when the variable is gone, rather than showing a number", () => {
    expect(read([{ $var: "gone" }])).toEqual({
      kind: "bound",
      variable: { id: "gone", name: null, value: undefined },
    });
  });
  it("is mixed when layers differ, including a bound one next to a literal", () => {
    expect(read([8, 12])).toEqual({ kind: "mixed" });
    expect(read([{ $var: "v1" }, 16])).toEqual({ kind: "mixed" });
    expect(read([{ $var: "v1" }, { $var: "v2" }])).toEqual({ kind: "mixed" });
  });
  it("is none for no layers", () => {
    expect(read([])).toEqual({ kind: "none" });
  });
});

describe("describeValue", () => {
  it("is the chip text of the design: the name in braces, then the value", () => {
    expect(describeValue({ id: "v1", name: "space/4", value: 16 }, " px")).toBe("{space/4} 16");
    expect(describeValue({ id: "v3", name: "color/surface", value: "#FBF9F3" }, "")).toBe(
      "{color/surface} #FBF9F3",
    );
  });
  it("shows a missing variable as such", () => {
    expect(describeValue({ id: "x", name: null, value: undefined }, "")).toBe("Missing variable");
  });
});

it("resolves long valid alias chains without an arbitrary depth cut-off", () => {
  const chain: Document = {
    meta: { formatVersion: 1, name: "TEST DATA", createdWith: "test" },
    pages: [],
    variables: Object.fromEntries(
      Array.from({ length: 32 }, (_, i) => [
        `v${i}`,
        {
          type: "number" as const,
          name: `space/${i}`,
          values: [i === 31 ? { value: 42 } : { $alias: `v${i + 1}` }],
        },
      ]),
    ),
  };
  expect(resolveVariable(chain, "v0", {})).toBe(42);
});
