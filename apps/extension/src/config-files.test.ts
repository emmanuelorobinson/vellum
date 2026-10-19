// TEST DATA: filesystem boundaries are mocked; no real client file is read or changed.
import { beforeEach, expect, it, vi } from "vitest";

const io = vi.hoisted(() => ({ stat: vi.fn(), read: vi.fn(), mkdir: vi.fn(), write: vi.fn() }));
vi.mock("node:fs/promises", () => ({
  lstat: io.stat,
  readFile: io.read,
  mkdir: io.mkdir,
  access: vi.fn(),
}));
vi.mock("./files", () => ({ files: { writeAtomic: io.write } }));
vi.mock("vscode", () => ({
  Uri: { file: (path: string) => ({ toString: () => `file://${path}` }) },
}));

import { readConfig, writeConfig } from "./config-files";

beforeEach(() => {
  vi.resetAllMocks();
  io.stat.mockResolvedValue({ isFile: () => true, isSymbolicLink: () => false, size: 2 });
  io.read.mockResolvedValue(Buffer.from("{}"));
});
it("distinguishes a missing config from denied/corrupt files without exposing contents", async () => {
  io.stat.mockRejectedValueOnce({ code: "ENOENT" });
  expect(await readConfig("/config")).toBeNull();
  io.stat.mockRejectedValueOnce({ code: "EACCES", message: "TEST DATA private" });
  await expect(readConfig("/config")).rejects.toThrow("Configuration unavailable");
  io.read.mockResolvedValueOnce(Buffer.from([0xff]));
  await expect(readConfig("/config")).rejects.toThrow("Configuration unavailable");
});
it("refuses symlink/nonfile/oversized config targets", async () => {
  for (const stat of [
    { isFile: () => true, isSymbolicLink: () => true, size: 2 },
    { isFile: () => false, isSymbolicLink: () => false, size: 2 },
    { isFile: () => true, isSymbolicLink: () => false, size: 5 * 1024 * 1024 },
  ]) {
    io.stat.mockResolvedValueOnce(stat);
    await expect(readConfig("/config")).rejects.toThrow();
  }
  expect(io.read).not.toHaveBeenCalled();
});
it("refuses a concurrent edit and commits only through the atomic file port", async () => {
  await expect(writeConfig("/client/config.json", "updated", "old")).rejects.toThrow(
    "Configuration changed",
  );
  expect(io.write).not.toHaveBeenCalled();
  await writeConfig("/client/config.json", "updated", "{}");
  expect(io.write).toHaveBeenCalledWith(
    "file:///client/config.json",
    new TextEncoder().encode("updated"),
  );
});
