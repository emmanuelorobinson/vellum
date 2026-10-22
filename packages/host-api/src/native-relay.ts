/** Stdio to private IPC. The bundled Rust bridge owns authentication, safe run-dir checks and daemon startup. */
import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";
import { type ShareRequest, type ShareState, shareState } from "./share";
export interface SessionRow {
  id: string;
  path: string;
  title: string;
  client: string;
  updatedAt: number;
  turn?: { seq: number; kind: "working" | "done" | "failed" | "permission" | "stopped" } | null;
}
export interface HostState {
  clients: string[];
  toolCount: number;
  sessions: SessionRow[];
  sessionsAvailable: boolean;
}
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
  private listeners = new Set<(connected: boolean) => void>();
  constructor(
    private readonly binary: string,
    private readonly cwd: string,
    private readonly runDir?: string,
    private readonly clientName = "vellum-vscode",
  ) {}
  get connected() {
    return this.ready;
  }
  onConnection(listener: (connected: boolean) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private connection(ready: boolean) {
    if (this.ready === ready) return;
    this.ready = ready;
    for (const listener of this.listeners) listener(ready);
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
          if (v.error) {
            // The daemon's own message (never forwarded to a webview; hosts show their own copy).
            const message = (v.error as { message?: unknown }).message;
            request.reject(Error(typeof message === "string" ? message : "daemon request failed"));
          } else request.resolve(v.result);
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
        clientInfo: { name: this.clientName, version: "0.0.0" },
      });
      if (
        this.child !== child ||
        !initialized ||
        typeof initialized !== "object" ||
        (initialized as { protocolVersion?: string }).protocolVersion !== "2025-11-25"
      )
        throw Error("offline");
      this.connection(true);
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
  /** The daemon's `vellum/editor` host method (ADR-033): an editor's replica of a document. */
  async editor(params: Record<string, string>): Promise<Record<string, unknown>> {
    await this.connect();
    const result = await this.request("vellum/editor", params);
    if (!result || typeof result !== "object" || Array.isArray(result))
      throw Error("invalid editor reply");
    return result as Record<string, unknown>;
  }
  async pauseAll(): Promise<void> {
    await this.connect();
    const result = (await this.request("vellum/pauseAll", {})) as { paused?: boolean };
    if (result?.paused !== true) throw Error("pause all was not acknowledged");
  }
  async recovery(
    path: string,
    action: "find" | "save" | "saveAs" | "discard" | "closeHeadless",
    destination?: string,
  ): Promise<{ savedAt?: number | null }> {
    await this.connect();
    const result = await this.request("vellum/recovery", {
      path,
      action,
      ...(destination ? { destination } : {}),
    });
    if (!result || typeof result !== "object") throw Error("invalid recovery");
    if (action === "find") {
      const savedAt = (result as { savedAt?: unknown }).savedAt;
      if (savedAt !== null && !Number.isSafeInteger(savedAt)) throw Error("invalid recovery");
      return { savedAt: savedAt as number | null };
    }
    if ((result as { done?: boolean }).done !== true) throw Error("recovery not acknowledged");
    return {};
  }
  async share(document: string, request: ShareRequest): Promise<ShareState> {
    await this.connect();
    return shareState(await this.request("vellum/share", { document, ...request }));
  }
  async hostState(paths: readonly string[]): Promise<HostState> {
    await this.connect();
    const state = await this.request("vellum/hostState", { paths });
    if (!state || typeof state !== "object") throw Error("invalid host state");
    const s = state as HostState;
    if (
      !Array.isArray(s.clients) ||
      !s.clients.every((c) => typeof c === "string") ||
      !Number.isSafeInteger(s.toolCount) ||
      s.toolCount < 0 ||
      typeof s.sessionsAvailable !== "boolean" ||
      !Array.isArray(s.sessions) ||
      s.sessions.length > 100 ||
      !s.sessions.every(
        (r) =>
          r &&
          typeof r.id === "string" &&
          typeof r.path === "string" &&
          paths.includes(r.path) &&
          typeof r.title === "string" &&
          typeof r.client === "string" &&
          Number.isSafeInteger(r.updatedAt) &&
          (!r.turn ||
            (Number.isSafeInteger(r.turn.seq) &&
              ["working", "done", "failed", "permission", "stopped"].includes(r.turn.kind))),
      )
    )
      throw Error("invalid host state");
    return s;
  }
  disconnect() {
    this.connection(false);
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
    this.listeners.clear();
  }
}
