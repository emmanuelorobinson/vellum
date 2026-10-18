// TEST DATA: filesystem boundary simulation; exercise failure before atomic replacement.
import { beforeEach, describe, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({
  lstat: vi.fn(),
  open: vi.fn(),
  readFile: vi.fn(),
  rename: vi.fn(),
  unlink: vi.fn(),
}));
vi.mock("node:fs/promises", () => io);
vi.mock("vscode", () => ({
  Uri: {
    parse: (uri: string) => ({
      scheme: uri.startsWith("file:") ? "file" : "https",
      fsPath: "/repo/design.vellum",
    }),
  },
}));

import { files } from "./files";

beforeEach(() => {
  for (const mock of Object.values(io)) mock.mockReset();
});
describe("native atomic writes", () => {
  it("flushes and closes the exclusive temporary file before replacing the source", async () => {
    const order: string[] = [];
    io.lstat.mockResolvedValue({ isFile: () => true, isSymbolicLink: () => false, mode: 0o100640 });
    io.open.mockResolvedValue({
      writeFile: async () => {
        order.push("write");
      },
      sync: async () => {
        order.push("sync");
      },
      close: async () => {
        order.push("close");
      },
    });
    io.rename.mockImplementation(async () => {
      order.push("rename");
    });
    io.unlink.mockRejectedValue(Object.assign(Error(), { code: "ENOENT" }));
    await files.writeAtomic("file:///repo/design.vellum", new Uint8Array([1]));
    expect(order).toEqual(["write", "sync", "close", "rename"]);
    expect(io.open).toHaveBeenCalledWith(
      expect.stringContaining("/repo/.vellum-save-"),
      "wx",
      0o640,
    );
    expect(io.rename).toHaveBeenCalledWith(expect.any(String), "/repo/design.vellum");
  });
  it("failed serialization to disk closes and removes the temporary file without touching the source", async () => {
    const close = vi.fn();
    io.lstat.mockRejectedValue(Object.assign(Error(), { code: "ENOENT" }));
    io.open.mockResolvedValue({
      writeFile: async () => {
        throw Error("disk full");
      },
      close,
    });
    io.unlink.mockResolvedValue(undefined);
    await expect(
      files.writeAtomic("file:///repo/design.vellum", new Uint8Array([1])),
    ).rejects.toThrow("disk full");
    expect(close).toHaveBeenCalledOnce();
    expect(io.rename).not.toHaveBeenCalled();
    expect(io.unlink).toHaveBeenCalledOnce();
  });
  it("refuses symlinks and remote filesystem resources", async () => {
    io.lstat.mockResolvedValue({ isFile: () => true, isSymbolicLink: () => true });
    await expect(files.writeAtomic("file:///repo/design.vellum", new Uint8Array())).rejects.toThrow(
      "unsupported file",
    );
    await expect(files.read("https://example.org/file")).rejects.toThrow("unsupported filesystem");
    expect(io.open).not.toHaveBeenCalled();
    expect(io.readFile).not.toHaveBeenCalled();
  });
});
