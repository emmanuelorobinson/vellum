/** Native config I/O. No complete client file is ever returned to a webview or diagnostic. */
import { constants } from "node:fs";
import { access, lstat, mkdir, readFile } from "node:fs/promises";
import { delimiter, dirname, join } from "node:path";
import * as vscode from "vscode";
import { files } from "./files";

export async function readConfig(path: string): Promise<string | null> {
  try {
    const stat = await lstat(path);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 4 * 1024 * 1024)
      throw Error("unsupported configuration");
    return new TextDecoder("utf-8", { fatal: true }).decode(await readFile(path));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw Error("Configuration unavailable");
  }
}
export async function exists(path: string): Promise<boolean> {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}
export async function executable(name: string): Promise<boolean> {
  for (const folder of (process.env.PATH ?? "").split(delimiter)) {
    if (!folder) continue;
    for (const suffix of process.platform === "win32" ? [".exe", ".cmd", ".bat", ""] : [""]) {
      try {
        await access(join(folder, name + suffix), constants.X_OK);
        return true;
      } catch {
        /* Next PATH candidate. */
      }
    }
  }
  return false;
}
export async function writeConfig(
  path: string,
  text: string,
  expected: string | null,
): Promise<void> {
  await mkdir(dirname(path), { recursive: true, mode: 0o700 });
  if ((await readConfig(path)) !== expected)
    throw Error("Configuration changed. Retry after reviewing it.");
  await files.writeAtomic(vscode.Uri.file(path).toString(), new TextEncoder().encode(text));
}
