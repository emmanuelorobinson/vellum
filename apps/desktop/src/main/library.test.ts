// TEST DATA. Metadata and lifecycle contracts; no document implementation in TypeScript.
import { describe, expect, it, vi } from "vitest";
import { DesktopLibrary, type LibraryData } from "./library";

function setup(data?: LibraryData) {
  let persisted = data;
  let now = 10;
  let id = 0;
  const create = vi.fn(async (_path: string) => {});
  const move = vi.fn(async (_from: string, _to: string) => {});
  const remove = vi.fn(async (_path: string) => {});
  const persist = vi.fn(async (next: LibraryData) => {
    persisted = structuredClone(next);
  });
  const library = new DesktopLibrary({
    load: async () => persisted ?? null,
    persist,
    canonical: async (path) => path,
    inspect: async () => ({ created: 1, modified: 2 }),
    folderOf: (path) => path.slice(0, path.lastIndexOf("/")),
    nameOf: (path) => path.split("/").at(-1) ?? "",
    draftPath: (id) => `/private/drafts/${id}.vellum`,
    now: () => ++now,
    nextId: () => `item${++id}`,
    create,
    move,
    remove,
  });
  return { library, create, move, remove, persist, saved: () => persisted };
}
describe("desktop library", () => {
  it("deduplicates recents, persists restart state and never deletes a removed recent", async () => {
    const s = setup();
    await s.library.ready();
    const id = await s.library.opened("/project/a.vellum");
    expect(await s.library.opened("/project/a.vellum")).toBe(id);
    expect(s.library.view().files).toHaveLength(1);
    const restarted = setup(s.saved());
    await restarted.library.ready();
    expect(restarted.library.path(id)).toBe("/project/a.vellum");
    await restarted.library.forget([id]);
    expect(restarted.remove).not.toHaveBeenCalled();
    expect(restarted.library.view().files).toEqual([]);
  });
  it("creates drafts through core, retains failed Save As and only deletes confirmed drafts", async () => {
    const s = setup();
    await s.library.ready();
    const id = await s.library.newDraft();
    expect(s.create).toHaveBeenCalledWith(s.library.path(id));
    expect(s.library.view().files[0]?.kind).toBe("draft");
    await expect(s.library.deleteDraft(id, false)).resolves.toBe(false);
    expect(s.remove).not.toHaveBeenCalled();
    s.move.mockRejectedValueOnce(Error("disk full"));
    await expect(s.library.saveAs(id, "/project/saved.vellum")).rejects.toThrow();
    expect(s.library.view().files[0]?.kind).toBe("draft");
    await s.library.saveAs(id, "/project/saved.vellum");
    expect(s.library.path(id)).toBe("/project/saved.vellum");
    await expect(s.library.deleteDraft(id, true)).rejects.toThrow();
  });
  it("serializes concurrent opens and preserves state on metadata write failure", async () => {
    const s = setup();
    await s.library.ready();
    await Promise.all(Array.from({ length: 8 }, () => s.library.opened("/p/a.vellum")));
    expect(s.library.view().files).toHaveLength(1);
    s.persist.mockRejectedValueOnce(Error("disk full"));
    const id = s.library.view().files[0]?.id;
    if (!id) throw Error("missing file");
    await expect(s.library.forget([id])).rejects.toThrow("disk full");
    expect(s.library.path(id)).toBe("/p/a.vellum");
    await expect(s.library.forget(["foreign"])).rejects.toThrow();
    expect(s.library.view().files).toHaveLength(1);
  });
});
it("never evicts an auto-saved draft when recents fill up", async () => {
  const s = setup();
  await s.library.ready();
  const draft = await s.library.newDraft();
  for (let index = 0; index < 205; index++) await s.library.opened(`/project/${index}.vellum`);
  expect(s.library.path(draft)).toBe(`/private/drafts/${draft}.vellum`);
});
