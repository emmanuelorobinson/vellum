// TEST DATA: the Button / Juniper component of design screens C16 and C16-instance.
import type { Document, Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import type { SceneItem } from "../tool-types";
import {
  detachOps,
  readComponent,
  replaceInstanceOps,
  resetOverridePatch,
  resetPropPatch,
  resetVariantPatch,
  setComponentNameOps,
  setComponentPropOps,
  setInstancePropPatch,
  setVariantPatch,
} from "./component";

const doc = {
  meta: { formatVersion: 1, name: "t", createdWith: "t" },
  components: {
    c1: {
      name: "Button / Juniper",
      props: {
        "Show icon": { type: "boolean", default: true },
        Label: { type: "text", default: "Continue" },
        Count: { type: "number", default: 2 },
      },
      root: { type: "frame", id: "c1root", name: "Button", width: 120, height: 40 },
      variants: [
        { properties: { Variant: "Primary", Size: "Medium" } },
        { properties: { Variant: "Secondary", Size: "Medium" } },
        { properties: { Variant: "Primary", Size: "Small" } },
        { properties: { Variant: "Secondary", Size: "Small" } },
      ],
    },
    c2: {
      name: "Card",
      root: { type: "frame", id: "c2root", name: "Card", width: 100, height: 100 },
    },
  },
  imports: { Core: "libraries/core.vellum" },
  pages: [],
} as unknown as Document;

const item = (node: Record<string, unknown>, extra: Partial<SceneItem> = {}): SceneItem => ({
  node: { name: "n", width: 10, height: 10, ...node } as unknown as Node,
  bounds: { x: 0, y: 0, width: 10, height: 10 },
  children: [],
  parent: null,
  ...extra,
});
const instance = (extra: Record<string, unknown> = {}, scene: Partial<SceneItem> = {}) =>
  item({ type: "instance", id: "i1", componentId: "c1", ...extra }, scene);
const main = () => item({ type: "frame", id: "c1root", width: 120, height: 40 });

describe("readComponent", () => {
  it("is nothing for a selection that is not one component or instance", () => {
    expect(readComponent([], doc)).toBeNull();
    expect(readComponent([instance(), instance({ id: "i2" })], doc)).toBeNull();
    expect(readComponent([item({ type: "rect", id: "r1" })], doc)).toBeNull();
  });
  it("knows a main component by its root, and lists its props with their defaults", () => {
    const model = readComponent([main()], doc);
    expect(model).toMatchObject({ kind: "main", id: "c1", name: "Button / Juniper" });
    if (model?.kind !== "main") throw Error("main");
    expect(model.props).toEqual([
      { name: "Show icon", type: "boolean", value: true },
      { name: "Label", type: "text", value: "Continue" },
      { name: "Count", type: "number", value: 2 },
    ]);
    expect(model.axes).toEqual([
      { name: "Variant", values: ["Primary", "Secondary"] },
      { name: "Size", values: ["Medium", "Small"] },
    ]);
  });
  it("lists an instance's choices, and which of them it has changed", () => {
    const model = readComponent(
      [
        instance({
          variant: { Variant: "Secondary" },
          props: { Label: "Next step", "Show icon": false },
        }),
      ],
      doc,
    );
    if (model?.kind !== "instance") throw Error("instance");
    expect(model.component).toBe("Button / Juniper");
    expect(model.axes).toEqual([
      { name: "Variant", values: ["Primary", "Secondary"], value: "Secondary", overridden: true },
      { name: "Size", values: ["Medium", "Small"], value: "Medium", overridden: false },
    ]);
    expect(model.props).toEqual([
      { name: "Show icon", type: "boolean", value: false, default: true, overridden: true },
      { name: "Label", type: "text", value: "Next step", default: "Continue", overridden: true },
      { name: "Count", type: "number", value: 2, default: 2, overridden: false },
    ]);
  });
  it("lists the changes made inside the instance, with the name of what changed", () => {
    const model = readComponent(
      [
        instance({
          overrides: {
            "c1root/label": { props: { content: "x" } },
            "c1root/icon": { replace: { type: "rect", id: "r", name: "Icon" } },
          },
        }),
      ],
      doc,
    );
    if (model?.kind !== "instance") throw Error("instance");
    expect(model.overrides).toEqual([
      { path: "c1root/label", summary: "Changed properties" },
      { path: "c1root/icon", summary: "Replaced" },
    ]);
  });
  it("says where the main component is: here, or in a library", () => {
    const local = readComponent([instance()], doc);
    const lib = readComponent([instance({ library: "Core", componentId: "x9" })], doc);
    if (local?.kind !== "instance" || lib?.kind !== "instance") throw Error("instance");
    expect(local.library).toBeNull();
    expect(lib.library).toBe("Core");
    // A library's component is not in this file: its props cannot be listed here.
    expect(lib.props).toEqual([]);
    expect(lib.component).toBe("x9");
  });
  it("says a component that is gone is gone", () => {
    const model = readComponent([instance({ componentId: "gone" })], doc);
    expect(model).toMatchObject({ kind: "instance", missing: true });
  });
  it("marks an instance inside another instance, which cannot be detached from here", () => {
    const model = readComponent(
      [instance({}, { instance: { id: "outer", path: "outer/i1" } })],
      doc,
    );
    expect(model).toMatchObject({ kind: "instance", nested: true });
  });
});

describe("instance patches", () => {
  it("sets a variant axis, keeping the others", () => {
    expect(
      setVariantPatch(instance({ variant: { Size: "Small" } }), "Variant", "Secondary"),
    ).toEqual({
      variant: { Size: "Small", Variant: "Secondary" },
    });
  });
  it("resets an axis, and the whole choice when none is left", () => {
    expect(
      resetVariantPatch(instance({ variant: { Size: "Small", Variant: "Secondary" } }), "Size"),
    ).toEqual({
      variant: { Variant: "Secondary" },
    });
    expect(resetVariantPatch(instance({ variant: { Size: "Small" } }), "Size")).toEqual({
      variant: null,
    });
  });
  it("sets and resets props", () => {
    expect(setInstancePropPatch(instance({ props: { Label: "a" } }), "Show icon", false)).toEqual({
      props: { Label: "a", "Show icon": false },
    });
    expect(resetPropPatch(instance({ props: { Label: "a", Count: 1 } }), "Label")).toEqual({
      props: { Count: 1 },
    });
    expect(resetPropPatch(instance({ props: { Label: "a" } }), "Label")).toEqual({ props: null });
  });
  it("resets a change made inside the instance", () => {
    const i = instance({ overrides: { a: { props: {} }, b: { props: {} } } });
    expect(resetOverridePatch(i, "a")).toEqual({ overrides: { b: { props: {} } } });
    expect(resetOverridePatch(instance({ overrides: { a: { props: {} } } }), "a")).toEqual({
      overrides: null,
    });
  });
  it("is nothing for a layer that is not an instance", () => {
    expect(setVariantPatch(main(), "Variant", "Primary")).toBeNull();
    expect(resetPropPatch(main(), "Label")).toBeNull();
  });
});

describe("replaceInstanceOps", () => {
  it("points an instance at another component and drops what belonged to the old one", () => {
    const ops = replaceInstanceOps(
      instance({
        variant: { Size: "Small" },
        props: { Label: "a" },
        overrides: { a: { props: {} } },
      }),
      { componentId: "c2", library: null },
    );
    expect(ops).toEqual([
      {
        type: "update",
        id: "i1",
        patch: {
          componentId: "c2",
          library: null,
          variant: null,
          props: null,
          overrides: null,
          slotContent: null,
        },
      },
    ]);
  });
  it("keeps the library when the new component comes from one", () => {
    const [op] = replaceInstanceOps(instance(), { componentId: "k1", library: "Core" });
    expect(op).toMatchObject({ patch: { componentId: "k1", library: "Core" } });
  });
  it("replaces an instance inside another instance through an override", () => {
    const nested = instance(
      { variant: { Size: "Small" } },
      { instance: { id: "outer", path: "outer/i1" } },
    );
    const [op] = replaceInstanceOps(nested, { componentId: "c2", library: null });
    expect(op).toMatchObject({ type: "replaceInInstance", id: "outer", path: "outer/i1" });
    const node = (op as { node: Record<string, unknown> }).node;
    expect(node).toMatchObject({ componentId: "c2", id: "i1" });
    expect("variant" in node).toBe(false);
  });
  it("is nothing for a layer that is not an instance, or one pointed at the same component", () => {
    expect(replaceInstanceOps(main(), { componentId: "c2", library: null })).toEqual([]);
    expect(replaceInstanceOps(instance(), { componentId: "c1", library: null })).toEqual([]);
  });
});

describe("detachOps", () => {
  it("is the core's Detach for an instance", () => {
    expect(detachOps(instance())).toEqual([{ type: "detach", id: "i1" }]);
  });
  it("is nothing for a nested instance, or a layer that is not an instance", () => {
    expect(detachOps(instance({}, { instance: { id: "o", path: "o/i1" } }))).toEqual([]);
    expect(detachOps(main())).toEqual([]);
  });
});

describe("main component edits", () => {
  it("renames a component, sending its props and variants as they are", () => {
    const [op] = setComponentNameOps(doc, "c1", "Button / Juniper 2");
    expect(op).toMatchObject({
      type: "updateComponent",
      id: "c1",
      name: "Button / Juniper 2",
      props: { Label: { type: "text", default: "Continue" } },
    });
    expect((op as { variants: unknown[] }).variants).toHaveLength(4);
  });
  it("changes a prop's default, and refuses a value of the wrong kind", () => {
    const [op] = setComponentPropOps(doc, "c1", "Label", "Next");
    expect((op as { props: Record<string, unknown> }).props.Label).toEqual({
      type: "text",
      default: "Next",
    });
    expect(setComponentPropOps(doc, "c1", "Label", 3)).toEqual([]);
    expect(setComponentPropOps(doc, "c1", "Show icon", false)).toHaveLength(1);
    expect(setComponentPropOps(doc, "nope", "Label", "x")).toEqual([]);
    expect(setComponentPropOps(doc, "c1", "nope", "x")).toEqual([]);
  });
  it("does not rename to nothing", () => {
    expect(setComponentNameOps(doc, "c1", "  ")).toEqual([]);
  });
});
