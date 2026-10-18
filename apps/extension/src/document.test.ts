// TEST DATA: scripted core boundary. No TypeScript document mutations.
import { describe, expect, it, vi } from "vitest";
import { DesignDocument, type DocumentRuntime, type FilePort } from "./document";

function setup() {
  let bytes = new Uint8Array([1]);
  let changed = (_change: { revision: number }) => {};
  const calls: string[] = [];
  const runtime: DocumentRuntime = {
    editable: true,
    revision: 0,
    snapshot: vi.fn(async () => ({ bytes: bytes.slice(), revision: runtime.revision })),
    replace: vi.fn(async (_data, reason) => {
      calls.push(reason);
      runtime.revision++;
      return { revision: runtime.revision };
    }),
    subscribe: (listener) => {
      changed = listener;
      return { dispose() {} };
    },
    dispose: vi.fn(),
  };
  const files = new Map<string, Uint8Array>([["file", bytes.slice()]]);
  const io: FilePort = {
    read: async (uri) => {
      const b = files.get(uri);
      if (!b) throw Error("missing");
      return b.slice();
    },
    writeAtomic: vi.fn(async (uri, data) => {
      files.set(uri, data.slice());
    }),
    delete: vi.fn(async (uri) => {
      files.delete(uri);
    }),
  };
  const doc = new DesignDocument("file", runtime, io);
  const edit = () => {
    bytes = new Uint8Array([2]);
    runtime.revision++;
    changed({ revision: runtime.revision });
  };
  return { doc, runtime, io, files, edit, calls };
}

describe("custom document lifecycle", () => {
  it("only accepted runtime commits mark dirty; save uses core serialization", async () => {
    const s = setup();
    const changes = vi.fn();
    s.doc.subscribe(changes);
    expect(s.doc.dirty).toBe(false);
    s.edit();
    expect(s.doc.dirty).toBe(true);
    expect(changes).toHaveBeenCalledOnce();
    await s.doc.save();
    expect(s.files.get("file")).toEqual(new Uint8Array([2]));
    expect(s.doc.dirty).toBe(false);
  });
  it("backup preserves dirty state, restores through file Ops and can be deleted", async () => {
    const s = setup();
    s.edit();
    const backup = await s.doc.backup("backup");
    expect(s.doc.dirty).toBe(true);
    expect(s.files.get("file")).toEqual(new Uint8Array([1]));
    await s.doc.restore(backup.id);
    expect(s.calls).toEqual(["recovery"]);
    expect(s.doc.dirty).toBe(true);
    await backup.delete();
    expect(s.files.has("backup")).toBe(false);
  });
  it("revert and external changes ask core to apply attributed file operations", async () => {
    const s = setup();
    s.edit();
    await s.doc.revert();
    expect(s.calls).toEqual(["revert"]);
    expect(s.doc.dirty).toBe(false);
    s.files.set("file", new Uint8Array([3]));
    await s.doc.external();
    expect(s.calls).toEqual(["revert", "external"]);
  });
  it("does not overwrite a file on cancellation or a failed snapshot", async () => {
    const s = setup();
    s.edit();
    await expect(s.doc.save(() => true)).rejects.toThrow("cancelled");
    expect(s.io.writeAtomic).not.toHaveBeenCalled();
    expect(s.doc.dirty).toBe(true);
    vi.mocked(s.runtime.snapshot).mockRejectedValueOnce(Error("failed"));
    await expect(s.doc.save()).rejects.toThrow();
    expect(s.io.writeAtomic).not.toHaveBeenCalled();
  });
  it("does not mark concurrent edits saved when serialization was older", async () => {
    const s = setup();
    s.edit();
    vi.mocked(s.io.writeAtomic).mockImplementationOnce(async () => {
      s.edit();
    });
    await s.doc.save();
    expect(s.doc.dirty).toBe(true);
  });
  it("failed writes and external decode failure retain dirty state and the last good base", async () => {
    const s = setup();
    s.edit();
    vi.mocked(s.io.writeAtomic).mockRejectedValueOnce(Error("read-only"));
    await expect(s.doc.save()).rejects.toThrow();
    expect(s.doc.dirty).toBe(true);
    vi.mocked(s.runtime.replace).mockRejectedValueOnce(Error("unreadable"));
    await expect(s.doc.external()).rejects.toThrow();
    expect(s.doc.dirty).toBe(true);
  });
  it("ignores self writes and serializes concurrent lifecycle requests", async () => {
    const s = setup();
    s.edit();
    await s.doc.save();
    await s.doc.external();
    expect(s.runtime.replace).not.toHaveBeenCalled();
    await Promise.all([s.doc.revert(), s.doc.backup("backup")]);
    expect(s.calls).toEqual(["revert"]);
  });
  it("Save As does not clear the old resource's dirty bit", async () => {
    const s = setup();
    s.edit();
    await s.doc.saveAs("other");
    expect(s.files.get("other")).toEqual(new Uint8Array([2]));
    expect(s.doc.dirty).toBe(true);
  });
  it("a person edit arriving after a revert commit remains dirty", async () => {
    const s = setup();
    s.edit();
    vi.mocked(s.runtime.replace).mockImplementationOnce(async () => {
      const revision = ++s.runtime.revision;
      s.edit();
      return { revision };
    });
    await s.doc.revert();
    expect(s.doc.dirty).toBe(true);
  });
  it("a read-only runtime cannot overwrite its source file", async () => {
    const s = setup();
    Object.defineProperty(s.runtime, "editable", { value: false });
    await expect(s.doc.save()).rejects.toThrow("read-only");
    expect(s.io.writeAtomic).not.toHaveBeenCalled();
  });
  it("disposes the runtime exactly once and refuses later work", async () => {
    const s = setup();
    s.doc.dispose();
    s.doc.dispose();
    expect(s.runtime.dispose).toHaveBeenCalledOnce();
    await expect(s.doc.save()).rejects.toThrow("disposed");
  });
});
