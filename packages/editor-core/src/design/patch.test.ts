import type { Node } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import type { SceneItem } from "../tool-types";
import { applyPatch, editTransaction, patchOps } from "./patch";

const node = (extra: Record<string, unknown> = {}) =>
  ({ type: "frame", id: "a", name: "A", width: 10, height: 10, ...extra }) as unknown as Node;
const item = (n: Node, extra: Partial<SceneItem> = {}): SceneItem => ({
  node: n,
  bounds: { x: 0, y: 0, width: 10, height: 10 },
  children: [],
  parent: null,
  ...extra,
});

describe("applyPatch", () => {
  it("sets properties", () => {
    expect(applyPatch(node(), { opacity: 0.5 })).toMatchObject({ opacity: 0.5 });
  });
  it("puts a property back to its default with null, as the core does", () => {
    const out = applyPatch(node({ opacity: 0.5, stroke: { width: 1, paints: [] } }), {
      opacity: null,
    });
    expect("opacity" in out).toBe(false);
    expect(out).toMatchObject({ stroke: { width: 1 } });
  });
  it("does not touch the id, the kind or the children", () => {
    const out = applyPatch(node({ children: [] }), {
      id: "zzz",
      type: "rect",
      children: [{}],
    } as never);
    expect(out).toMatchObject({ id: "a", type: "frame", children: [] });
  });
  it("does not change the node it was given", () => {
    const n = node();
    applyPatch(n, { opacity: 0.1 });
    expect("opacity" in n).toBe(false);
  });
});

describe("patchOps", () => {
  it("updates a layer by id", () => {
    expect(patchOps([item(node())], () => ({ opacity: 0.5 }))).toEqual([
      { type: "update", id: "a", patch: { opacity: 0.5 } },
    ]);
  });
  it("skips layers the patch does not apply to", () => {
    expect(patchOps([item(node())], () => null)).toEqual([]);
  });
  it("replaces inside the instance for a part of an instance, so the component is never written", () => {
    const inner = node({ id: "inner" });
    const ops = patchOps([item(inner, { instance: { id: "inst", path: "inst/inner" } })], () => ({
      opacity: 0.5,
    }));
    expect(ops).toEqual([
      {
        type: "replaceInInstance",
        id: "inst",
        path: "inst/inner",
        node: { ...inner, opacity: 0.5 },
      },
    ]);
  });
  it("honours null inside an instance too", () => {
    const inner = node({ id: "inner", opacity: 0.5 });
    const [op] = patchOps([item(inner, { instance: { id: "i", path: "i/inner" } })], () => ({
      opacity: null,
    }));
    expect(op).toMatchObject({ type: "replaceInInstance" });
    expect("opacity" in ((op as { node: object }).node as object)).toBe(false);
  });
});

describe("editTransaction", () => {
  it("is one Person transaction for the whole selection, so one undo step", () => {
    const tx = editTransaction("dev", [item(node()), item(node({ id: "b" }))], () => ({
      opacity: 0.5,
    }));
    expect(tx?.origin).toEqual({ actor: { kind: "person", device: "dev" } });
    expect(tx?.ops).toHaveLength(2);
  });
  it("is nothing when there is nothing to change", () => {
    expect(editTransaction("dev", [item(node())], () => null)).toBeNull();
    expect(editTransaction("dev", [], () => ({ opacity: 1 }))).toBeNull();
  });
});
