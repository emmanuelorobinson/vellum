import { randomUUID } from "node:crypto";
import { constants } from "node:fs";
import {
  copyFile,
  lstat,
  mkdir,
  open,
  readdir,
  readFile,
  realpath,
  rename,
  unlink,
} from "node:fs/promises";
import { dirname, join } from "node:path";
import type { LibraryData } from "./library";

export async function privateDirectory(path: string) {
  await mkdir(path, { recursive: true, mode: 0o700 });
  const info = await lstat(path);
  if (
    !info.isDirectory() ||
    info.isSymbolicLink() ||
    (process.platform !== "win32" && (info.mode & 0o077) !== 0)
  )
    throw Error("invalid private folder");
  return realpath(path);
}
export async function readMetadata(path: string): Promise<LibraryData | null> {
  try {
    const info = await lstat(path);
    if (!info.isFile() || info.isSymbolicLink() || info.size > 4 * 1024 * 1024)
      throw Error("invalid metadata");
    const data = JSON.parse(await readFile(path, "utf8")) as LibraryData;
    const id = (s: unknown) => typeof s === "string" && /^[a-zA-Z0-9_-]{1,80}$/.test(s);
    if (
      data.version !== 1 ||
      !Array.isArray(data.files) ||
      data.files.length > 400 ||
      !Array.isArray(data.folders) ||
      data.folders.length > 20 ||
      !data.files.every(
        (f) =>
          f &&
          id(f.id) &&
          typeof f.path === "string" &&
          f.path.endsWith(".vellum") &&
          typeof f.name === "string" &&
          typeof f.folder === "string" &&
          ["file", "draft"].includes(f.kind) &&
          Number.isFinite(f.created) &&
          Number.isFinite(f.opened),
      ) ||
      !data.folders.every((f) => f && id(f.id) && typeof f.path === "string")
    )
      throw Error("invalid metadata");
    return data;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw Error("Home metadata unavailable");
  }
}
export async function atomicMetadata(path: string, data: unknown) {
  return atomicBytes(path, JSON.stringify(data));
}
export async function atomicBytes(path: string, text: string) {
  const temporary = `${path}.${randomUUID()}.new`;
  const file = await open(temporary, "wx", 0o600);
  try {
    await file.writeFile(text);
    await file.sync();
    await file.close();
    await rename(temporary, path);
  } finally {
    await file.close().catch(() => {});
    await unlink(temporary).catch(() => {});
  }
}
/** Relocates canonical bytes without interpreting or changing the document. Never overwrites. */
export async function moveDraft(from: string, to: string) {
  const source = await realpath(from);
  if (source !== from || !(await lstat(source)).isFile()) throw Error("invalid");
  const parent = await realpath(dirname(to));
  if (parent !== dirname(to)) throw Error("invalid");
  await copyFile(from, to, constants.COPYFILE_EXCL);
  const file = await open(to, "r+");
  try {
    await file.sync();
  } finally {
    await file.close();
  }
}
export async function removeDraft(path: string, draftDirectory: string) {
  if (dirname(path) !== draftDirectory || (await realpath(path)) !== path) throw Error("invalid");
  await unlink(path);
}
export const metadataPath = (profile: string) => join(profile, "home.json");

/** Recover a core-created draft if a process died before the metadata rename committed. */
export async function recoverDraftIndex(
  data: LibraryData | null,
  directory: string,
): Promise<LibraryData> {
  const next = data ?? { version: 1 as const, files: [], folders: [] };
  for (const name of await readdir(directory)) {
    const id = /^([a-f0-9-]{36})\.vellum$/.exec(name)?.[1];
    if (!id) continue;
    const path = join(directory, name);
    const info = await lstat(path);
    if (!info.isFile() || info.isSymbolicLink() || next.files.some((f) => f.path === path))
      continue;
    if (next.files.length >= 400) throw Error("draft index limit reached");
    next.files.push({
      id,
      path,
      name: "Untitled",
      folder: directory,
      kind: "draft",
      created: Math.trunc(info.birthtimeMs),
      opened: Math.trunc(info.mtimeMs),
      branch: null,
      running: null,
    });
  }
  return next;
}
