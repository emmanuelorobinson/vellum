// TEST DATA: scripts are boundary responses, not a TypeScript DocStore or model provider.
import { describe, expect, it } from "vitest";
import { FakeEngineClient, type ToolTransaction, WasmEngineClient } from "./index";

const transaction: ToolTransaction = {
  origin: { actor: { kind: "person", device: "device-test" } },
  ops: [{ type: "scale", id: "shape00001", factor: 2, anchor: { x: 0, y: 0 } }],
};
describe("typed WASM boundary", () => {
  it("forwards exactly one Person transaction, actor undo/redo and isolated copies", () => {
    const binding = new FakeEngineClient();
    const engine = new WasmEngineClient(binding);
    binding.script({ revision: 1, readOnly: false, document: null });
    expect(engine.apply(transaction)).toEqual({ status: "applied", revision: 1 });
    expect(binding.transactions).toEqual([transaction]);
    expect(binding.transactions[0]).not.toBe(transaction);
    for (const operation of ["undo", "redo"] as const) {
      binding.script(
        { revision: operation === "undo" ? 2 : 3, readOnly: false, document: null },
        operation,
      );
      engine[operation](transaction.origin.actor);
    }
    expect(binding.historyActors).toEqual([transaction.origin.actor, transaction.origin.actor]);
    engine.dispose();
    expect(binding.state).toBe("disposed");
  });
  it("protects snapshots from subscribers and refuses invalid / unscripted / read-only writes", () => {
    const engine = new FakeEngineClient({
      revision: 0,
      readOnly: false,
      document: { meta: { formatVersion: 1, name: "Contract", createdWith: "fixture" }, pages: [] },
    });
    engine.subscribe((snapshot) => {
      if (snapshot.document) snapshot.document.meta.name = "mutated";
    });
    expect(engine.apply(transaction)).toEqual({ status: "rejected", reason: "unavailable" });
    engine.script({ ...engine.snapshot(), revision: 1 });
    engine.apply(transaction);
    expect(engine.snapshot().document?.meta.name).toBe("Contract");
    const copy = engine.snapshot();
    if (copy.document) copy.document.meta.name = "mutated";
    expect(engine.snapshot().document?.meta.name).toBe("Contract");
    const readonly = new WasmEngineClient(
      new FakeEngineClient({ revision: 0, document: null, readOnly: true }),
    );
    expect(readonly.apply(transaction)).toEqual({ status: "rejected", reason: "read-only" });
    engine.dispose();
    readonly.dispose();
  });
  it("Scene results are copies and a retained facade refuses queries after disposal", () => {
    const roots = ["shape00001"];
    const fake = new FakeEngineClient();
    const binding = new FakeEngineClient(undefined, { ...fake.scene, roots: () => roots });
    const engine = new WasmEngineClient(binding);
    const scene = engine.scene;
    scene.roots().push("mutated");
    expect(roots).toEqual(["shape00001"]);
    engine.dispose();
    expect(() => scene.roots()).toThrow("disposed");
    fake.dispose();
  });
});

it("forwards segment hits through the retained Scene facade without exposing mutable records", () => {
  const target = { type: "segment" as const, contour: 0, index: 1, t: 0.5 };
  const fake = new FakeEngineClient();
  const bindings = new FakeEngineClient(undefined, {
    ...fake.scene,
    hitPath: (_address, point) => {
      point.x = 999;
      return target;
    },
  });
  const engine = new WasmEngineClient(bindings);
  const point = { x: 10, y: 20 };
  const hit = engine.scene.hitPath?.("path", point, 4);
  expect(hit).toEqual(target);
  if (hit) hit.index = 7;
  expect(target.index).toBe(1);
  expect(point).toEqual({ x: 10, y: 20 });
});
