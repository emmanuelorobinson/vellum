import { randomBytes } from "node:crypto";
import { lstat, open, readFile, rename, unlink } from "node:fs/promises";
import { dirname, join } from "node:path";
import * as vscode from "vscode";
import type { FilePort } from "./document";

function path(uri: string) {
  const parsed = vscode.Uri.parse(uri);
  if (parsed.scheme !== "file") throw Error("unsupported filesystem");
  return parsed.fsPath;
}
export const files: FilePort = {
  read: async (uri) => new Uint8Array(await readFile(path(uri))),
  async writeAtomic(uri, bytes) {
    const target = path(uri);
    let mode = 0o600;
    try {
      const stat = await lstat(target);
      if (!stat.isFile() || stat.isSymbolicLink()) throw Error("unsupported file");
      mode = stat.mode & 0o777;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
    const temp = join(dirname(target), `.vellum-save-${randomBytes(16).toString("hex")}`);
    try {
      const file = await open(temp, "wx", mode);
      try {
        await file.writeFile(bytes);
        await file.sync();
      } finally {
        await file.close();
      }
      await rename(temp, target);
    } finally {
      await unlink(temp).catch((error: NodeJS.ErrnoException) => {
        if (error.code !== "ENOENT") throw Error("temporary file cleanup failed");
      });
    }
  },
  async delete(uri) {
    await unlink(path(uri)).catch((error: NodeJS.ErrnoException) => {
      if (error.code !== "ENOENT") throw Error("backup cleanup failed");
    });
  },
};
