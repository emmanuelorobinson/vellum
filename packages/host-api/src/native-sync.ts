/** Native-only bridge. Fixed argv carries a keychain entry name; no secret reaches the editor. */
import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import { isAbsolute } from "node:path";
import type { ClientFrame } from "@vellum/types-ts";
import { RpcFrames } from "./native-relay";
import { parseSyncFrame, type SyncWire } from "./sync";
export interface NativeSyncOptions {
  binary: string;
  endpoint: string;
  room: string;
  key: string;
}
export class NativeSyncWire implements SyncWire {
  private child: ChildProcessWithoutNullStreams | null = null;
  constructor(private options: NativeSyncOptions) {
    const u = new URL(options.endpoint);
    if (
      !isAbsolute(options.binary) ||
      u.username ||
      u.password ||
      u.search ||
      u.hash ||
      u.pathname !== "/" ||
      (u.protocol !== "wss:" &&
        !(u.protocol === "ws:" && ["127.0.0.1", "localhost", "[::1]"].includes(u.hostname))) ||
      !/^[A-Za-z0-9_-]{1,64}$/.test(options.room) ||
      !/^[A-Za-z0-9_-]{1,128}$/.test(options.key)
    )
      throw Error("Invalid sync configuration");
  }
  async open(onFrame: (frame: unknown) => void, onClose: () => void) {
    this.close();
    const p = this.options;
    const child = spawn(
      p.binary,
      ["sync", "--endpoint", p.endpoint, "--room", p.room, "--key", p.key],
      { shell: false, windowsHide: true, stdio: "pipe" },
    );
    this.child = child;
    const frames = new RpcFrames(192 * 1024 * 1024);
    const fail = () => {
      if (this.child === child) {
        this.child = null;
        child.kill();
        onClose();
      }
    };
    child.on("error", fail);
    child.on("exit", fail);
    child.stdin.on("error", fail);
    child.stderr.resume();
    child.stdout.on("data", (chunk: Buffer) => {
      if (this.child !== child) return;
      try {
        for (const value of frames.push(chunk)) {
          const f = parseSyncFrame(value);
          if (!f) throw Error("Invalid sync frame");
          onFrame(f);
        }
      } catch {
        fail();
      }
    });
    child.stdout.on("end", () => {
      try {
        frames.end();
      } catch {
        fail();
      }
    });
    await new Promise<void>((resolve, reject) => {
      child.once("spawn", resolve);
      child.once("error", () => reject(Error("Sync unavailable")));
    });
  }
  send(frame: ClientFrame) {
    if (!this.child || this.child.stdin.writableLength > 16 * 1024 * 1024)
      throw Error("Sync unavailable");
    const text = JSON.stringify(frame);
    if (Buffer.byteLength(text) > 9 * 1024 * 1024) throw Error("Sync frame too large");
    this.child.stdin.write(`${text}\n`);
  }
  close() {
    const child = this.child;
    this.child = null;
    if (child) {
      child.stdin.end();
      child.kill();
    }
  }
}
