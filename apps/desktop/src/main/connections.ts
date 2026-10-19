import { constants } from "node:fs";
import { access, lstat, mkdir, readFile, realpath } from "node:fs/promises";
import { homedir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { AgentConnections } from "@vellum/host-vscode/agent-connections";
import { atomicBytes } from "./storage";

async function exists(path: string) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}
async function read(path: string) {
  try {
    const info = await lstat(path);
    if (!info.isFile() || info.isSymbolicLink() || info.size > 4 * 1024 * 1024)
      throw Error("invalid");
    return new TextDecoder("utf-8", { fatal: true }).decode(await readFile(path));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw Error("unavailable");
  }
}
export function connections(binary: string, appData: string, workspace?: string) {
  return new AgentConnections({
    home: homedir(),
    platform: process.platform,
    configHome: process.env.XDG_CONFIG_HOME ?? join(homedir(), ".config"),
    appData,
    workspace,
    binary,
    exists,
    read,
    codexHome: process.env.CODEX_HOME,
    claudeConfig: process.env.CLAUDE_CONFIG_DIR
      ? join(process.env.CLAUDE_CONFIG_DIR, ".claude.json")
      : undefined,
    async executable(name) {
      for (const directory of (process.env.PATH ?? "").split(delimiter).filter(Boolean))
        for (const suffix of process.platform === "win32" ? [".exe", ".cmd", ".bat", ""] : [""])
          try {
            await access(join(directory, name + suffix), constants.X_OK);
            return true;
          } catch {
            /* next */
          }
      return false;
    },
    async write(path, text, expected) {
      await mkdir(dirname(path), { recursive: true, mode: 0o700 });
      if ((await realpath(dirname(path))) !== dirname(path) || (await read(path)) !== expected)
        throw Error("configuration changed");
      await atomicBytes(path, text);
    },
  });
}
