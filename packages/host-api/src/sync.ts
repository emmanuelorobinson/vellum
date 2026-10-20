/** Credential-free editor transport. The native host owns the authenticated socket and OS keychain. */
import type { ClientFrame, ServerFrame, SyncReply, SyncRequest } from "@vellum/types-ts";
import type { Disposable } from "./host";
import type { SyncClient } from "./services";
export interface SyncWire {
  open(onFrame: (value: unknown) => void, onClose: () => void): Promise<void>;
  send(frame: ClientFrame): void;
  close(): void;
}
export type SyncStateBody = Extract<SyncReply, { type: "state" }>;
export interface SyncEngine {
  /** Atomic import into the shared Rust engine; includes canonical bootstrap, never a TS document store. */
  apply(state: SyncStateBody): Promise<void>;
  /** Accepted local Rust transactions only. Imports must never echo into this subscription. */
  subscribeLocal?(listener: (update: Uint8Array) => void): Disposable;
}
export interface SyncRecovery {
  epoch: number;
  revision: number;
  requests: SyncRequest[];
  blocked?: number[];
}
export interface SyncOutbox {
  /** Atomic host-owned storage. No credentials. Native history must be persisted with its matching cursor. */
  save(state: SyncRecovery): Promise<void>;
}
type Pending = {
  request: SyncRequest;
  blocked?: boolean;
  resolve(): void;
  reject(reason: Error): void;
};
export class RoomSyncClient implements SyncClient {
  private status: SyncClient["state"] = "offline";
  private at = { epoch: 0, revision: 0 };
  private queue: Pending[] = [];
  private inflight: Pending | null = null;
  private incoming = Promise.resolve();
  private listeners = new Set<(update: Uint8Array) => void>();
  private changes = new Set<() => void>();
  private messages = new Set<(frame: ServerFrame) => void>();
  private generation = 0;
  private opening: Promise<void> | null = null;
  private local: Disposable | undefined;
  constructor(
    private wire: SyncWire,
    private engine: SyncEngine,
    private outbox?: SyncOutbox,
    restored?: SyncRecovery,
  ) {
    if (restored) {
      this.at = { epoch: restored.epoch, revision: restored.revision };
      this.queue = cloneFrame(restored.requests).map((request, index) => ({
        request,
        blocked: restored.blocked?.includes(index) ?? false,
        resolve() {},
        reject() {},
      }));
    }
    this.local = engine.subscribeLocal?.((update) => {
      void this.send(update).catch(() => {});
    });
  }
  get state() {
    return this.status;
  }
  get pending() {
    return this.queue.length;
  }
  get cursor() {
    return { ...this.at };
  }
  get persistent() {
    return !!this.outbox;
  }
  recovery(): SyncRecovery {
    return cloneFrame({
      ...this.at,
      requests: this.queue.map((p) => p.request),
      blocked: this.queue.flatMap((p, i) => (p.blocked ? [i] : [])),
    });
  }
  private notify() {
    for (const l of [...this.changes]) l();
  }
  private change(s: SyncClient["state"]) {
    this.status = s;
    this.notify();
  }
  private save() {
    return this.outbox?.save(this.recovery()) ?? Promise.resolve();
  }
  async connect(): Promise<void> {
    if (this.state === "disposed") throw Error("Sync disposed");
    if (["needs-review", "incompatible", "forbidden"].includes(this.state))
      throw Error("Sync needs review");
    if (this.opening) return this.opening;
    if (this.state === "ready") return;
    this.change("connecting");
    const generation = ++this.generation;
    this.opening = this.wire
      .open(
        (v) => {
          this.incoming = this.incoming
            .then(async () => {
              if (generation === this.generation) await this.receive(v);
            })
            .catch(() => {
              if (generation === this.generation) this.change("needs-review");
            });
        },
        () => {
          if (generation === this.generation && this.state !== "disposed") {
            this.inflight = null;
            this.change("offline");
          }
        },
      )
      .then(() => {
        if (generation !== this.generation) return;
        this.frame({ type: "pull", after: this.at.revision });
      })
      .catch(() => {
        this.change("offline");
        throw Error("Sync unavailable");
      })
      .finally(() => {
        this.opening = null;
      });
    return this.opening;
  }
  private frame(request: SyncRequest) {
    this.wire.send({ v: 1, adapter: "loro-1", epoch: this.at.epoch, request });
  }
  send(update: Uint8Array): Promise<void> {
    if (!update.length || update.length > 2 * 1024 * 1024)
      return Promise.reject(Error("Invalid sync update"));
    return this.request({ type: "push", bytes: [...update] });
  }
  request(request: SyncRequest): Promise<void> {
    if (this.state === "disposed" || this.queue.length >= 256)
      return Promise.reject(Error("Sync unavailable"));
    const result = new Promise<void>((resolve, reject) => {
      this.queue.push({
        request: cloneFrame(request),
        blocked:
          request.type === "push" && this.queue.some((p) => p.blocked && p.request.type === "push"),
        resolve,
        reject,
      });
    });
    this.notify();
    this.incoming = this.incoming
      .then(() => this.save())
      .then(() => this.flush())
      .catch(() => this.change("needs-review"));
    return result;
  }
  /** Presence is replaceable ephemeral data; never enters the durable edit outbox. */
  ephemeral(request: SyncRequest) {
    if (this.state === "ready") this.frame(request);
  }
  private flush() {
    const next = this.queue.find((p) => !p.blocked);
    if (this.state !== "ready" || this.inflight || !next) return;
    this.inflight = next;
    try {
      this.frame(next.request);
    } catch {
      this.inflight = null;
      this.change("offline");
    }
  }
  private async receive(value: unknown) {
    const f = parseSyncFrame(value);
    if (!f) {
      this.change("incompatible");
      return;
    }
    if (f.v !== 1 || f.adapter !== "loro-1") {
      this.change("incompatible");
      return;
    }
    if (f.body.type === "state") {
      if (
        (this.at.epoch !== 0 && f.epoch !== this.at.epoch) ||
        (this.at.epoch === 0 && this.queue.some((p) => p.request.type === "push"))
      ) {
        this.change("needs-review");
        return;
      }
      if (f.epoch === this.at.epoch && f.revision < this.at.revision) return;
      await this.engine.apply(f.body);
      this.at = { epoch: f.epoch, revision: f.revision };
      await this.save();
      for (const bytes of f.body.updates)
        for (const l of [...this.listeners]) l(new Uint8Array(bytes));
      this.change("ready");
      this.flush();
    } else if (f.body.type === "ack" || f.body.type === "sharing") {
      if (f.epoch !== this.at.epoch || !this.inflight) return;
      if ((this.inflight.request.type === "sharing") !== (f.body.type === "sharing")) {
        this.change("needs-review");
        return;
      }
      const accepted = this.inflight;
      const index = this.queue.indexOf(accepted);
      this.queue.splice(index, 1);
      this.inflight = null;
      try {
        await this.save();
        accepted.resolve();
      } catch {
        this.queue.splice(index, 0, accepted);
        this.change("needs-review");
        accepted.reject(Error("Sync recovery unavailable"));
        return;
      }
      this.notify();
      this.flush();
    } else if (f.body.type === "error") {
      const denied = this.inflight;
      this.inflight = null;
      if (
        denied &&
        ((f.body.code === "forbidden" && denied.request.type === "push") ||
          (denied.request.type !== "push" &&
            !["reset_required", "upgrade_required"].includes(f.body.code)))
      ) {
        if (denied.request.type === "push")
          for (const p of this.queue) {
            if (p.request.type === "push") p.blocked = true;
          }
        else this.queue.splice(this.queue.indexOf(denied), 1);
        denied.reject(Error("Sync request was not accepted"));
        await this.save();
        this.notify();
        this.flush();
        for (const l of [...this.messages]) l(cloneFrame(f));
        return;
      }
      this.change(
        f.body.code === "forbidden"
          ? "forbidden"
          : f.body.code === "upgrade_required"
            ? "incompatible"
            : f.body.code === "reset_required" || f.body.code === "invalid_update"
              ? "needs-review"
              : "offline",
      );
      denied?.reject(Error("Sync request was not accepted"));
    }
    for (const l of [...this.messages]) l(cloneFrame(f));
  }
  idle() {
    return this.incoming;
  }
  subscribe(listener: (update: Uint8Array) => void): Disposable {
    this.listeners.add(listener);
    return { dispose: () => void this.listeners.delete(listener) };
  }
  subscribeState(listener: () => void): Disposable {
    this.changes.add(listener);
    return { dispose: () => void this.changes.delete(listener) };
  }
  subscribeFrames(listener: (frame: ServerFrame) => void): Disposable {
    this.messages.add(listener);
    return { dispose: () => void this.messages.delete(listener) };
  }
  disconnect() {
    this.generation++;
    this.wire.close();
    this.inflight = null;
    if (this.state !== "disposed") this.change("offline");
  }
  dispose() {
    if (this.state === "disposed") return;
    this.disconnect();
    this.local?.dispose();
    this.change("disposed");
    for (const q of this.queue) q.reject(Error("Sync disposed"));
    this.listeners.clear();
    this.changes.clear();
    this.messages.clear();
  }
}
function bytes(value: unknown): value is number[] {
  return Array.isArray(value) && value.every((n) => Number.isInteger(n) && n >= 0 && n <= 255);
}
export function parseSyncFrame(v: unknown): ServerFrame | null {
  if (!v || typeof v !== "object") return null;
  const f = v as ServerFrame;
  if (
    !Number.isSafeInteger(f.v) ||
    typeof f.adapter !== "string" ||
    !Number.isSafeInteger(f.epoch) ||
    f.epoch < 0 ||
    !Number.isSafeInteger(f.revision) ||
    f.revision < 0 ||
    !f.body ||
    typeof f.body !== "object"
  )
    return null;
  if (f.v !== 1 || f.adapter !== "loro-1") return f;
  const b = f.body;
  if (
    b.type === "state" &&
    (b.snapshot === null || (bytes(b.snapshot) && b.snapshot.length <= 32 * 1024 * 1024)) &&
    Array.isArray(b.updates) &&
    b.updates.length <= 256 &&
    b.updates.every((x) => bytes(x) && x.length <= 2 * 1024 * 1024)
  )
    return f;
  if (
    b.type === "ack" ||
    (b.type === "error" && typeof b.code === "string" && /^[a-z_]{1,64}$/.test(b.code))
  )
    return f;
  if (
    b.type === "sharing" &&
    b.state &&
    actor(b.state.owner_actor) &&
    typeof b.state.owner === "boolean" &&
    typeof b.state.enabled === "boolean" &&
    typeof b.state.can_live === "boolean" &&
    Array.isArray(b.state.members) &&
    b.state.members.length <= 1024 &&
    b.state.members.every(
      (m) => actor(m.actor) && ["viewer", "commenter", "editor"].includes(m.role),
    )
  )
    return f;
  if (
    b.type === "collaboration" &&
    b.grant &&
    ["viewer", "commenter", "editor"].includes(b.grant.role) &&
    actor(b.grant.actor) &&
    Array.isArray(b.participants) &&
    b.participants.length <= 1024 &&
    b.participants.every(
      (p) =>
        typeof p.connection === "string" &&
        p.connection.length <= 128 &&
        actor(p.actor) &&
        Number.isSafeInteger(p.expires_ms) &&
        (!p.state ||
          (typeof p.state.name === "string" &&
            p.state.name.length <= 128 &&
            typeof p.state.activity === "string" &&
            p.state.activity.length <= 256 &&
            (!p.state.cursor ||
              (Number.isFinite(p.state.cursor.x) && Number.isFinite(p.state.cursor.y))) &&
            (!p.state.viewport ||
              (Number.isFinite(p.state.viewport.x) &&
                Number.isFinite(p.state.viewport.y) &&
                p.state.viewport.zoom > 0 &&
                p.state.viewport.zoom <= 256)) &&
            (!p.state.hue ||
              ["teal", "saffron", "green", "orange", "rose", "magenta", "lime", "sand"].includes(
                p.state.hue,
              )))),
    )
  )
    return f;
  return null;
}
function actor(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const a = value as { kind?: string; device?: string; agent?: string };
  const id = a.kind === "person" ? a.device : a.kind === "agent" ? a.agent : undefined;
  return typeof id === "string" && /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/.test(id);
}

function cloneFrame<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
