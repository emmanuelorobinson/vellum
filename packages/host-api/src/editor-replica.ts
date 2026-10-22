/**
 * An editor as a replica of the daemon's engine (ADR-033). The editor's Rust engine applies the person's edits
 * at once; this loop pushes them to the daemon as sync updates and pulls everyone else's (agents', other
 * editors', the file's) on an interval. Requests go one at a time over the host's `vellum/editor` wire. It
 * carries bytes only: no document is built or changed here.
 */
export interface ReplicaEngine {
  cursor(): Uint8Array;
  updatesSince(cursor: Uint8Array): Uint8Array;
  /** Returns whether anything changed. */
  import(updates: Uint8Array): boolean;
}

/** The daemon's `vellum/editor` method for one open editor; the host adds the path and editor name. */
export type EditorWire = (
  action: "open" | "push" | "pull" | "close",
  params: Record<string, string>,
) => Promise<Record<string, unknown>>;

export interface EditorStart {
  role: "writer" | "attached";
  genesis: Uint8Array;
  updates: Uint8Array;
  revision: number;
  /** A backup from a session that ended unexpectedly (A06). */
  recovery: { savedAt: number } | null;
}

/** The platform globals this file needs, typed here so packages without DOM types can compile it. */
const platform = globalThis as unknown as {
  btoa(text: string): string;
  atob(text: string): string;
  TextEncoder: new () => { encode(text: string): Uint8Array };
  setInterval(run: () => void, ms: number): unknown;
  clearInterval(timer: unknown): void;
};

export function toBase64(bytes: Uint8Array): string {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return platform.btoa(text);
}

export function fromBase64(text: string): Uint8Array {
  const raw = platform.atob(text);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return bytes;
}

const field = (reply: Record<string, unknown>, key: string) => {
  const value = reply[key];
  if (typeof value !== "string") throw Error(`The daemon's reply has no ${key}`);
  return value;
};

/** Opens the editor at the daemon: the genesis document and everything since, for `WasmDocumentEngine.open`. */
export async function openReplica(wire: EditorWire, device?: string): Promise<EditorStart> {
  // A relaying host may add the device itself, so the webview never names one.
  const reply = await wire("open", device ? { device } : {});
  const role = reply.role;
  if (role !== "writer" && role !== "attached") throw Error("The daemon's reply has no role");
  return {
    role,
    genesis: new platform.TextEncoder().encode(field(reply, "genesis")),
    updates: fromBase64(field(reply, "updates")),
    revision: Number(reply.revision ?? 0),
    recovery: (reply.recovery as EditorStart["recovery"]) ?? null,
  };
}

export class EditorReplica {
  /** What the daemon has of this replica's history, as far as this loop knows. */
  private remote: Uint8Array;
  private queue: Promise<unknown> = Promise.resolve();
  private timer: unknown;
  private closed = false;
  /** The daemon refused or could not be reached; the host shows it and the editor goes read-only. */
  failure: string | null = null;

  constructor(
    private readonly engine: ReplicaEngine,
    private readonly wire: EditorWire,
    private readonly options: {
      interval?: number;
      /** The daemon's engine revision after each exchange (dirty state for the host). */
      onRevision?: (revision: number) => void;
      onFailure?: (message: string) => void;
    } = {},
  ) {
    this.remote = engine.cursor();
  }

  private serial<T>(step: () => Promise<T>): Promise<T> {
    const next = this.queue.then(step);
    this.queue = next.catch(() => {});
    return next;
  }

  private fail(error: unknown) {
    this.failure = error instanceof Error ? error.message : String(error);
    this.stop();
    this.options.onFailure?.(this.failure);
  }

  private revision(reply: Record<string, unknown>) {
    if (typeof reply.revision === "number") this.options.onRevision?.(reply.revision);
  }

  /** Starts pulling. */
  start() {
    if (this.timer || this.closed) return;
    this.timer = platform.setInterval(() => void this.pull(), this.options.interval ?? 250);
  }

  stop() {
    if (this.timer) platform.clearInterval(this.timer);
    this.timer = undefined;
  }

  /** Sends what the person changed since the last push. Call after every commit, undo and redo. */
  push(): Promise<void> {
    return this.serial(async () => {
      if (this.closed || this.failure) return;
      const updates = this.engine.updatesSince(this.remote);
      if (!updates.byteLength) return;
      const cursor = this.engine.cursor();
      try {
        this.revision(await this.wire("push", { updates: toBase64(updates) }));
        this.remote = cursor;
      } catch (error) {
        this.fail(error);
      }
    });
  }

  /** Takes what others changed. Returns whether the document changed. */
  pull(): Promise<boolean> {
    return this.serial(async () => {
      if (this.closed || this.failure) return false;
      try {
        const reply = await this.wire("pull", { cursor: toBase64(this.engine.cursor()) });
        const changed = this.engine.import(fromBase64(field(reply, "updates")));
        this.revision(reply);
        return changed;
      } catch (error) {
        this.fail(error);
        return false;
      }
    });
  }

  /** Pushes anything left, then closes the editor at the daemon. */
  async close() {
    if (this.closed) return;
    await this.push();
    this.stop();
    this.closed = true;
    await this.serial(() => this.wire("close", {}).catch(() => ({})));
  }
}
