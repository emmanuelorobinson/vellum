// TEST DATA: host results and engine replies are scripted; no second document store.
import { FakeEngineClient } from "@vellum/engine-client";
import type { Document } from "@vellum/types-ts";
import { describe, expect, it } from "vitest";
import { createResourceController, type ResourceHost, shellOf } from "./controller";

const document = (): Document => ({
  meta: { formatVersion: 1, name: "Test", createdWith: "test" },
  themes: { Mode: ["Light", "Dark"] },
  styles: {},
  fonts: {},
  imports: { core: "../old.lib.vellum" },
  pages: [],
});
function fixture(host: ResourceHost = {}) {
  const engine = new FakeEngineClient({ revision: 0, document: document(), readOnly: false });
  const c = createResourceController({
    engine,
    device: "test-device",
    host,
    nextVariableId: () => "new",
  });
  return {
    engine,
    c,
    reply: (d: Document = document()) =>
      engine.script({ document: d, revision: 1, readOnly: false }),
  };
}
describe("resource controller", () => {
  it("preserves every shell field when editing one", () => {
    const { c, engine, reply } = fixture();
    reply();
    c.setThemes({ Mode: ["Light", "Dark", "Dim"] });
    expect(engine.transactions[0]).toMatchObject({
      origin: { actor: { kind: "person", device: "test-device" } },
      ops: [
        {
          type: "setShell",
          shell: { ...shellOf(document()), themes: { Mode: ["Light", "Dark", "Dim"] } },
        },
      ],
    });
  });
  it("creates a typed variable with an injected id", () => {
    const { c, engine, reply } = fixture();
    reply();
    c.createVariable("space/8", "number", "32");
    expect(engine.transactions[0]?.ops).toEqual([
      {
        type: "setVariables",
        set: { new: { type: "number", name: "space/8", values: [{ value: 32 }] } },
        remove: [],
      },
    ]);
  });
  it("refuses duplicate variable names", () => {
    const { c } = fixture();
    expect(() => c.createVariable(" ", "number", "0")).toThrow();
  });
  it("locates a missing library, changes only the import, and offers save only after apply", async () => {
    const { c, reply, engine } = fixture({
      chooseLibrary: async () => ({ alias: "core", path: "../new.lib.vellum" }),
      save: async () => true,
    });
    reply();
    await c.locate("core");
    expect(engine.transactions[0]?.ops).toMatchObject([
      { type: "setShell", shell: { imports: { core: "../new.lib.vellum" } } },
    ]);
    expect(c.state.saveSuggested).toBe(true);
    await c.save();
    expect(c.state.saveSuggested).toBe(false);
  });
  it("reports refusal and never offers save when locate fails", async () => {
    const { c } = fixture({
      chooseLibrary: async () => ({ alias: "core", path: "../new.lib.vellum" }),
    });
    await c.locate("core");
    expect(c.state.error).toBeTruthy();
    expect(c.state.saveSuggested).toBe(false);
  });
  it("treats cancellation as no write", async () => {
    const { c, engine } = fixture({ chooseLibrary: async () => null });
    await c.locate("core");
    expect(engine.transactions).toHaveLength(0);
  });
  it.each(["/tmp/a.lib.vellum", "C:\\a.lib.vellum", "../a.vellum", "https://x/a.lib.vellum"])(
    "refuses invalid import path %s",
    async (path) => {
      const { c, engine } = fixture({ chooseLibrary: async () => ({ alias: "other", path }) });
      await c.importLibrary();
      expect(c.state.error).toBeTruthy();
      expect(engine.transactions).toHaveLength(0);
    },
  );
  it("does not overwrite a same-named import", async () => {
    const { c, engine } = fixture({
      chooseLibrary: async () => ({ alias: "core", path: "../new.lib.vellum" }),
    });
    await c.importLibrary();
    expect(c.state.error).toBeTruthy();
    expect(engine.transactions).toHaveLength(0);
  });
  it("asks before conversion, then commits the library kind once", () => {
    const { c, engine, reply } = fixture();
    c.requestConversion();
    expect(engine.transactions).toHaveLength(0);
    expect(c.state.dialog).toBe("convert");
    reply();
    c.confirmConversion();
    expect(engine.transactions[0]?.ops).toMatchObject([
      { type: "setShell", shell: { meta: { kind: "library" } } },
    ]);
  });
  it("does not convert without an explicit confirmation request", () => {
    const { c, engine } = fixture();
    c.confirmConversion();
    expect(engine.transactions).toHaveLength(0);
  });
  it("imports asset metadata returned by the host, preserving other shell fields", async () => {
    const { c, reply, engine } = fixture({
      chooseFont: async () => ({ asset: "a".repeat(64), face: { family: "Custom", weight: 500 } }),
    });
    reply();
    await c.importFont();
    expect(engine.transactions[0]?.ops).toMatchObject([
      {
        type: "setShell",
        shell: { fonts: { ["a".repeat(64)]: { family: "Custom", weight: 500 } } },
      },
    ]);
  });
  it("ignores a stale file result after a document change", async () => {
    let resolve!: (v: { alias: string; path: string }) => void;
    const { c, engine, reply } = fixture({
      chooseLibrary: () =>
        new Promise((r) => {
          resolve = r;
        }),
    });
    const pending = c.importLibrary();
    reply({ ...document(), meta: { ...document().meta, name: "Other" } });
    engine.apply({
      origin: { actor: { kind: "person", device: "test-device" } },
      ops: [{ type: "setShell", shell: shellOf(document()) }],
    });
    resolve({ alias: "other", path: "other.lib.vellum" });
    await pending;
    expect(engine.transactions).toHaveLength(1);
    expect(c.state.error).toContain("changed");
  });
  it("does not open external pickers in read-only files", async () => {
    let calls = 0;
    const { c, engine } = fixture({
      chooseLibrary: async () => {
        calls++;
        return null;
      },
    });
    engine.script({ document: document(), revision: 1, readOnly: true });
    engine.apply({
      origin: { actor: { kind: "person", device: "test-device" } },
      ops: [{ type: "setShell", shell: shellOf(document()) }],
    });
    await c.importLibrary();
    expect(calls).toBe(0);
  });
  it("closing a dialog invalidates a pending file result", async () => {
    let resolve!: (v: { alias: string; path: string }) => void;
    const { c, engine } = fixture({
      chooseLibrary: () =>
        new Promise((r) => {
          resolve = r;
        }),
    });
    c.open("libraries");
    const pending = c.importLibrary();
    c.close();
    resolve({ alias: "other", path: "other.lib.vellum" });
    await pending;
    expect(engine.transactions).toHaveLength(0);
  });
});

it("a failed save keeps the save offer and reports its error", async () => {
  const { c, reply } = fixture({
    chooseLibrary: async () => ({ alias: "core", path: "new.lib.vellum" }),
    save: async () => {
      throw Error("Save failed");
    },
  });
  reply();
  await c.locate("core");
  await c.save();
  expect(c.state.saveSuggested).toBe(true);
  expect(c.state.error).toBe("Save failed");
});
it("paste refuses an incompatible Use existing decision before the planner runs", async () => {
  let calls = 0;
  const { c, engine } = fixture({
    planPaste: async () => {
      calls++;
      return [];
    },
  });
  c.requestPaste({
    id: "clip",
    conflicts: [{ id: "v", name: "space/4", existing: "16", pasted: "text", sameType: false }],
    omissions: [],
  });
  await c.paste({ link: false, variables: { v: "existing" } });
  expect(calls).toBe(0);
  expect(engine.transactions).toHaveLength(0);
  expect(c.state.error).toContain("Resolve");
});
it("a rejected paste keeps its decisions available to retry", async () => {
  const { c } = fixture({
    planPaste: async () => [{ type: "setShell", shell: shellOf(document()) }],
  });
  c.requestPaste({ id: "clip", conflicts: [], omissions: [] });
  await c.paste({ link: false, variables: {} });
  expect(c.state.dialog).toBe("paste");
  expect(c.state.error).toContain("refused");
});

it("pastes copied component definitions and their instances through one Person transaction", async () => {
  const component = {
    name: "TEST DATA",
    root: { type: "frame" as const, id: "root", name: "Main", width: 100, height: 40 },
  };
  const { c, engine, reply } = fixture({
    planPaste: async () => [
      { type: "setComponent", id: "copied", component },
      {
        type: "insert",
        to: { parent: { type: "page", id: "p1" }, index: 0 },
        node: {
          type: "instance",
          id: "instance",
          name: "Copy",
          componentId: "copied",
          width: 100,
          height: 40,
        },
      },
    ],
  });
  c.requestPaste({ id: "clip", conflicts: [], omissions: [] });
  reply();
  await c.paste({ link: false, variables: {} });
  expect(engine.transactions).toHaveLength(1);
  expect(engine.transactions[0]).toMatchObject({
    origin: { actor: { kind: "person", device: "test-device" } },
    ops: [
      { type: "setComponent", id: "copied", component },
      { type: "insert", node: { componentId: "copied" } },
    ],
  });
});

it("a disposed controller refuses later document mutations", () => {
  const { c, engine } = fixture();
  c.dispose();
  expect(() => c.setThemes({})).toThrow("disposed");
  expect(engine.transactions).toHaveLength(0);
});
