import type { AgentId, SessionId, TurnId } from "@vellum/types-ts";
import { type Disposable, PortError } from "./host";

export type ConnectionState = "offline" | "ready" | "disposed";
export type DaemonAction =
  | { type: "stop"; session: SessionId }
  | { type: "pauseAll" }
  | { type: "resumeAll" }
  | { type: "undoTurn"; turn: TurnId }
  | { type: "stopAgent"; agent: AgentId; turn: TurnId };
export interface DaemonEvent {
  type: "invalidate";
  document: string;
}
/** Relayed through the host. Session data is fetched through later typed feature ports, never raw RPC. */
export interface DaemonClient {
  readonly state: ConnectionState;
  connect(): Promise<void>;
  request(action: DaemonAction): Promise<void>;
  subscribe(listener: (event: DaemonEvent) => void): Disposable;
  disconnect(): void;
  dispose(): void;
}
export interface DaemonRelay {
  connect(): Promise<void>;
  request(action: DaemonAction): Promise<void>;
  subscribe(listener: (event: DaemonEvent) => void): Disposable;
  disconnect(): void;
}
export class RelayedDaemonClient implements DaemonClient {
  private connection: ConnectionState = "offline";
  private listeners = new Set<(e: DaemonEvent) => void>();
  private subscription: Disposable | null = null;
  private pending: Promise<void> | null = null;
  private generation = 0;
  get state() {
    return this.connection;
  }
  constructor(private readonly relay: DaemonRelay) {}
  connect(): Promise<void> {
    if (this.connection === "disposed") return Promise.reject(new PortError("disposed"));
    if (this.connection === "ready") return Promise.resolve();
    if (this.pending) return this.pending;
    const generation = this.generation;
    const pending = this.relay
      .connect()
      .then(() => {
        if (generation !== this.generation) {
          this.relay.disconnect();
          throw new PortError("offline");
        }
        this.connection = "ready";
        this.subscription = this.relay.subscribe((event) => {
          if (this.state === "ready")
            for (const listener of [...this.listeners]) listener({ ...event });
        });
      })
      .catch(() => {
        if (generation === this.generation && this.state !== "disposed")
          this.connection = "offline";
        throw new PortError("offline");
      })
      .finally(() => {
        if (this.pending === pending) this.pending = null;
      });
    this.pending = pending;
    return pending;
  }
  async request(action: DaemonAction): Promise<void> {
    if (this.state !== "ready")
      throw new PortError(this.state === "disposed" ? "disposed" : "offline");
    try {
      await this.relay.request({ ...action });
    } catch {
      this.disconnect();
      throw new PortError("offline");
    }
  }
  subscribe(listener: (event: DaemonEvent) => void): Disposable {
    if (this.state === "disposed") throw new PortError("disposed");
    this.listeners.add(listener);
    return {
      dispose: () => {
        this.listeners.delete(listener);
      },
    };
  }
  disconnect(): void {
    this.generation++;
    this.subscription?.dispose();
    this.subscription = null;
    this.relay.disconnect();
    if (this.state !== "disposed") this.connection = "offline";
  }
  dispose(): void {
    if (this.state === "disposed") return;
    this.disconnect();
    this.connection = "disposed";
    this.listeners.clear();
  }
}
/** Scriptable boundary fake; it never invokes a model, socket or timer. */
export class FakeDaemonClient implements DaemonClient {
  state: ConnectionState = "offline";
  readonly actions: DaemonAction[] = [];
  private listeners = new Set<(event: DaemonEvent) => void>();
  async connect() {
    if (this.state === "disposed") throw new PortError("disposed");
    this.state = "ready";
  }
  async request(action: DaemonAction) {
    if (this.state !== "ready") throw new PortError("offline");
    this.actions.push({ ...action });
  }
  subscribe(listener: (event: DaemonEvent) => void): Disposable {
    if (this.state === "disposed") throw new PortError("disposed");
    this.listeners.add(listener);
    return {
      dispose: () => {
        this.listeners.delete(listener);
      },
    };
  }
  emit(event: DaemonEvent) {
    if (this.state === "ready") for (const listener of [...this.listeners]) listener({ ...event });
  }
  disconnect() {
    if (this.state !== "disposed") this.state = "offline";
  }
  dispose() {
    this.state = "disposed";
    this.listeners.clear();
  }
}
export interface SyncClient {
  readonly state: "offline";
  connect(): Promise<void>;
  send(update: Uint8Array): Promise<void>;
  subscribe(listener: (update: Uint8Array) => void): Disposable;
  dispose(): void;
}
/** Local editing requires no sync connection. Refuse sends instead of pretending to publish. */
export class NoopSyncClient implements SyncClient {
  readonly state = "offline" as const;
  async connect() {}
  async send(_update: Uint8Array): Promise<void> {
    throw new PortError("offline");
  }
  subscribe(_listener: (update: Uint8Array) => void): Disposable {
    return { dispose() {} };
  }
  dispose() {}
}
export interface Telemetry {
  readonly enabled: boolean;
  event(name: string, metrics?: Readonly<Record<string, number>>): void;
  dispose(): void;
}
export class NoopTelemetry implements Telemetry {
  readonly enabled = false;
  event(_name: string, _metrics?: Readonly<Record<string, number>>) {}
  dispose() {}
}
