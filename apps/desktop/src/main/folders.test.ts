// TEST DATA: injected native file and document-runtime boundaries.
import { describe, expect, it, vi } from "vitest";
import { FolderWindows } from "./folders";

function setup() {
  let next = 0;
  const runtime = {
    open: vi.fn(async (_path: string) => ({ dirty: false, running: false })),
    save: vi.fn(async (_path: string) => {}),
    close: vi.fn(async (_path: string) => {}),
  };
  const model = new FolderWindows({
    canonical: async (path) => path.replace("/alias", "/repo"),
    folderOf: (path) => path.slice(0, path.lastIndexOf("/")),
    nameOf: (path) => path.slice(path.lastIndexOf("/") + 1),
    nextId: () => String(++next),
    runtime,
  });
  return { model, runtime };
}
describe("folder windows and document tabs", () => {
  it("scopes Windows paths using directory boundaries", async () => {
    let next = 0;
    const model = new FolderWindows({
      canonical: async (path) => path,
      folderOf: () => "",
      nameOf: (path) => path.split(/[\\/]/).at(-1) ?? "",
      nextId: () => String(++next),
      runtime: {
        open: async () => ({ dirty: false, running: false }),
        save: async () => {},
        close: async () => {},
      },
    });
    const folder = await model.openFolder("C:\\repo");
    const tab = await model.openDocument(folder, "C:\\repo\\nested\\a.vellum");
    expect(model.path(folder, tab)).toBe("C:\\repo\\nested\\a.vellum");
    await expect(model.openDocument(folder, "C:\\repo-other\\a.vellum")).rejects.toThrow("invalid");
  });
  it("deduplicates canonical folders and concurrent opens, preserving the active tab", async () => {
    const { model, runtime } = setup();
    const [a, b] = await Promise.all([model.openFolder("/repo"), model.openFolder("/alias")]);
    expect(a).toBe(b);
    const [first, duplicate] = await Promise.all([
      model.openDocument(a, "/repo/a.vellum"),
      model.openDocument(b, "/alias/a.vellum"),
    ]);
    expect(first).toBe(duplicate);
    expect(runtime.open).toHaveBeenCalledOnce();
    const second = await model.openDocument(a, "/repo/b.vellum");
    expect(model.state(a).active).toBe(second);
    model.activate(a, first);
    expect(model.state(a).active).toBe(first);
    const other = await model.openFolder("/other");
    expect(other).not.toBe(a);
    expect(() => model.activate(other, first)).toThrow("invalid");
  });
  it("scopes document capabilities to their window and refuses unsupported files", async () => {
    const { model } = setup();
    const a = await model.openFolder("/repo");
    const b = await model.openFolder("/other");
    const id = await model.openDocument(a, "/repo/sub/a.vellum");
    expect(model.path(a, id)).toBe("/repo/sub/a.vellum");
    expect(() => model.path(b, id)).toThrow("invalid");
    await expect(model.openDocument(a, "/repo/../other/a.vellum")).rejects.toThrow();
    await expect(model.openDocument(a, "/repo/key.txt")).rejects.toThrow();
    expect(JSON.stringify(model.state(a))).not.toContain("/repo");
  });
  it("retains dirty tabs on cancel or failed save, and selects the adjacent tab after close", async () => {
    const { model, runtime } = setup();
    const a = await model.openFolder("/repo");
    const first = await model.openDocument(a, "/repo/a.vellum");
    const second = await model.openDocument(a, "/repo/b.vellum");
    model.update(a, second, { dirty: true, running: true });
    expect(model.state(a).tabs[1]).toMatchObject({ dirty: true, running: true });
    expect(await model.closeTab(a, second, async () => "cancel")).toBe(false);
    runtime.save.mockRejectedValueOnce(Error("disk full"));
    await expect(model.closeTab(a, second, async () => "save")).rejects.toThrow();
    expect(model.state(a).tabs).toHaveLength(2);
    expect(await model.closeTab(a, second, async () => "discard")).toBe(true);
    expect(model.state(a).active).toBe(first);
    expect(() => model.path(a, second)).toThrow("invalid");
  });
  it("does not close any tabs when a window close is cancelled", async () => {
    const { model } = setup();
    const a = await model.openFolder("/repo");
    const one = await model.openDocument(a, "/repo/a.vellum");
    const two = await model.openDocument(a, "/repo/b.vellum");
    model.update(a, two, { dirty: true, running: false });
    expect(await model.closeWindow(a, async () => "cancel")).toBe(false);
    expect(model.state(a).tabs.map((t) => t.id)).toEqual([one, two]);
    expect(await model.closeWindow(a, async () => "discard")).toBe(true);
    expect(model.has(a)).toBe(false);
  });
  it("keeps a tab when it changes while its close confirmation is open", async () => {
    const { model } = setup();
    const folder = await model.openFolder("/repo");
    const tab = await model.openDocument(folder, "/repo/a.vellum");
    model.update(folder, tab, { dirty: true, running: false });
    const closed = await model.closeTab(folder, tab, async () => {
      model.update(folder, tab, { dirty: true, running: true });
      return "discard";
    });
    expect(closed).toBe(false);
    expect(model.path(folder, tab)).toBe("/repo/a.vellum");
  });
});
