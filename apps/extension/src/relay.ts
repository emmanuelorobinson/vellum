/** Stdio to private IPC. The bundled Rust bridge owns authentication, safe run-dir checks and daemon startup. */
import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
export class RpcFrames {
  private buffered = Buffer.alloc(0);
  constructor(private readonly limit = 16 * 1024 * 1024) {}
  push(chunk: Buffer): unknown[] {
    const output: unknown[] = [];
    // Scan first; never allocate more than one bounded frame for an untrusted stream.
    let start = 0;
    for (let i = 0; i < chunk.length; i++) {
      if (chunk[i] !== 10) continue;
      if (this.buffered.length + i - start > this.limit) throw Error("frame too large");
      const frame = Buffer.concat([this.buffered, chunk.subarray(start, i)]);
      this.buffered = Buffer.alloc(0);
      start = i + 1;
      output.push(JSON.parse(frame.toString("utf8")));
    }
    if (this.buffered.length + chunk.length - start > this.limit) throw Error("frame too large");
    this.buffered = Buffer.concat([this.buffered, chunk.subarray(start)]);
    return output;
  }
  end() {
    if (this.buffered.length) throw Error("truncated frame");
  }
}
type Pending = {
  resolve(value: unknown): void;
  reject(error: Error): void;
  timer: ReturnType<typeof setTimeout>;
};
/** No replay after a disconnect: the host cannot know whether a request was committed. */
export class DaemonProcess {
  private child: ChildProcessWithoutNullStreams | null = null;
  private pending = new Map<number, Pending>();
  private nextId = 0;
  private connecting: Promise<void> | null = null;
  private ready = false;
  private disposed = false;
  constructor(
    private readonly binary: string,
    private readonly cwd: string,
    private readonly runDir?: string,
  ) {}
  get connected() {
    return this.ready;
  }
  connect(): Promise<void> {
    if (this.disposed) return Promise.reject(Error("disposed"));
    if (this.ready) return Promise.resolve();
    if (this.connecting) return this.connecting;
    this.connecting = this.start().finally(() => {
      this.connecting = null;
    });
    return this.connecting;
  }
  private async start() {
    const child = spawn(this.binary, [...(this.runDir ? ["--run-dir", this.runDir] : []), "mcp"], {
      cwd: this.cwd,
      windowsHide: true,
      shell: false,
      stdio: "pipe",
    });
    this.child = child;
    const frames = new RpcFrames();
    const fail = () => {
      if (this.child === child) this.disconnect();
    };
    child.on("error", fail);
    child.on("exit", fail);
    child.stdin.on("error", fail);
    // Rust diagnostics can contain workspace paths; never forward stderr into the webview or logs.
    child.stderr.resume();
    child.stdout.on("data", (bytes: Buffer) => {
      if (this.child !== child) return;
      try {
        for (const value of frames.push(bytes)) {
          if (!value || typeof value !== "object") throw Error("invalid reply");
          const v = value as { jsonrpc?: string; id?: number; result?: unknown; error?: unknown };
          if (v.jsonrpc !== "2.0" || typeof v.id !== "number") throw Error("invalid reply");
          const request = this.pending.get(v.id);
          if (!request) continue;
          clearTimeout(request.timer);
          this.pending.delete(v.id);
          if (v.error) request.reject(Error("daemon request failed"));
          else request.resolve(v.result);
        }
      } catch {
        fail();
      }
    });
    child.stdout.on("end", () => {
      try {
        frames.end();
      } catch {
        /* Truncation is an offline result, never an uncaught exception. */
      } finally {
        fail();
      }
    });
    try {
      const initialized = await this.request("initialize", {
        protocolVersion: "2025-11-25",
        capabilities: {},
        clientInfo: { name: "vellum-vscode", version: "0.0.0" },
      });
      if (
        this.child !== child ||
        !initialized ||
        typeof initialized !== "object" ||
        (initialized as { protocolVersion?: string }).protocolVersion !== "2025-11-25"
      )
        throw Error("offline");
      this.ready = true;
    } catch {
      fail();
      throw Error("offline");
    }
  }
  private request(method: string, params: unknown): Promise<unknown> {
    const child = this.child;
    if (!child) return Promise.reject(Error("offline"));
    const id = ++this.nextId;
    const bytes = JSON.stringify({ jsonrpc: "2.0", id, method, params });
    if (Buffer.byteLength(bytes) > 1024 * 1024 - 1) return Promise.reject(Error("frame too large"));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.disconnect();
      }, 15_000);
      this.pending.set(id, { resolve, reject, timer });
      child.stdin.write(`${bytes}\n`, (error) => {
        if (error) this.disconnect();
      });
    });
  }
  async createDocument(path: string): Promise<void> {
    await this.connect();
    const reply = (await this.request("tools/call", {
      name: "open_document",
      arguments: { path, create: true },
    })) as { isError?: boolean };
    if (reply?.isError !== false) throw Error("document creation failed");
  }
  async pauseAll(): Promise<void> {
    await this.connect();
    const result = (await this.request("vellum/pauseAll", {})) as { paused?: boolean };
    if (result?.paused !== true) throw Error("pause all was not acknowledged");
  }
  disconnect() {
    this.ready = false;
    const child = this.child;
    this.child = null;
    child?.stdin.end();
    child?.kill(); // only this host's bridge, never the shared daemon
    for (const request of this.pending.values()) {
      clearTimeout(request.timer);
      request.reject(Error("offline"));
    }
    this.pending.clear();
  }
  dispose() {
    this.disposed = true;
    this.disconnect();
  }
}
