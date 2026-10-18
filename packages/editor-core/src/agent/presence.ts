import { copy } from "@vellum/config";
/** Presence is ephemeral daemon state. The Guard, rather than this UI, enforces pause and frame stops. */
import type { Box, EngineClient, ToolTransaction, Viewport } from "@vellum/engine-client";
import type { AgentId, NodeId, PageId, SessionId } from "@vellum/types-ts";
export interface PresentAgent {
  id: AgentId;
  name: string;
  hue: string;
  session: SessionId;
  state: "idle" | "working" | "paused" | "blocked_by_permission" | "failed" | "disconnected";
  engine?: string;
  task?: string;
  cursor?: { x: number; y: number };
  viewport?: Viewport;
}
export interface FrameLease {
  frame: NodeId;
  agent: AgentId;
  page: PageId;
  bounds: Box;
  changes: number;
  rejected: number;
  editing: boolean;
  idle?: boolean;
}
export interface InsertHighlight {
  node: NodeId;
  agent: AgentId;
  page: PageId;
  bounds: Box;
  expiresAt: number;
}
export interface PresenceSnapshot {
  connected: boolean;
  paused: boolean;
  agents: PresentAgent[];
  leases: FrameLease[];
  highlights: InsertHighlight[];
}
export type PresenceAction =
  | { type: "stopFrame"; agent: AgentId; frame: NodeId }
  | { type: "stopAgent"; agent: AgentId }
  | { type: "pauseAll" }
  | { type: "resumeAll" };
export interface PresenceFeed {
  read(): Promise<PresenceSnapshot>;
  subscribe(listener: (snapshot: PresenceSnapshot) => void): { dispose(): void };
  action(action: PresenceAction): Promise<void>;
}
export const presenceState = (s: PresentAgent["state"]) =>
  ({
    idle: "Idle",
    working: "Working",
    paused: "Paused",
    blocked_by_permission: "Needs permission",
    failed: "Failed",
    disconnected: "Disconnected",
  })[s];
export function leaseLabel(l: FrameLease): string {
  return l.editing
    ? copy.editingInsideLease
    : l.rejected
      ? `${l.rejected} changes rejected`
      : l.idle
        ? "Idle"
        : `Working · ${l.changes} changes`;
}
const EMPTY: PresenceSnapshot = {
  connected: false,
  paused: false,
  agents: [],
  leases: [],
  highlights: [],
};
export interface PresenceDeps {
  feed: PresenceFeed;
  now(): number;
  engine?: EngineClient;
  contains?(frame: NodeId, node: NodeId): boolean;
  onFollow(viewport: Viewport): void;
  onOpenSession?(session: SessionId): void;
  onRejected?(frame: NodeId, agent: AgentId): void;
}
export function createPresenceController(deps: PresenceDeps) {
  let state = {
    snapshot: EMPTY,
    following: null as AgentId | null,
    error: null as string | null,
    pending: false,
  };
  let disposed = false;
  let generation = 0;
  const edited = new Map<NodeId, number>();
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };
  const update = (snapshot: PresenceSnapshot) => {
    if (disposed) return;
    generation++;
    state = { ...state, snapshot: cloneJson(snapshot), error: null };
    for (const frame of edited.keys())
      if (!snapshot.leases.some((l) => l.frame === frame)) edited.delete(frame);
    const target = snapshot.connected
      ? snapshot.agents.find((a) => a.id === state.following && a.state !== "disconnected")
      : null;
    if (!target) state.following = null;
    else if (target.viewport) deps.onFollow(target.viewport);
    notify();
  };
  const sub = deps.feed.subscribe(update);
  const committed = (tx: ToolTransaction) => {
    const targets = tx.ops.flatMap((op) =>
      op.type === "insert"
        ? [op.node.id, op.to.parent.id]
        : op.type === "move"
          ? [op.id, op.to.parent.id]
          : "id" in op
            ? [op.id as NodeId]
            : [],
    );
    for (const lease of state.snapshot.leases)
      if (targets.some((node) => node === lease.frame || deps.contains?.(lease.frame, node)))
        edited.set(lease.frame, deps.now());
    notify();
  };
  const commits = deps.engine?.subscribeCommits?.(committed);
  return {
    get state() {
      return state;
    },
    async load() {
      const mine = generation;
      try {
        const snapshot = await deps.feed.read();
        if (!disposed && mine === generation) update(snapshot);
      } catch (e) {
        if (!disposed) {
          state = { ...state, snapshot: EMPTY, error: String(e) };
          notify();
        }
      }
    },
    agents: () =>
      state.snapshot.connected
        ? state.snapshot.agents.filter((a) => a.state !== "disconnected")
        : [],
    leases: () =>
      state.snapshot.connected
        ? state.snapshot.leases.map((l) => ({
            ...l,
            editing:
              l.editing || (edited.has(l.frame) && deps.now() - (edited.get(l.frame) ?? 0) < 5000),
          }))
        : [],
    tick() {
      notify();
    },
    highlights: () =>
      state.snapshot.connected
        ? state.snapshot.highlights.filter((h) => h.expiresAt > deps.now())
        : [],
    follow(agent: AgentId | null) {
      state = { ...state, following: agent };
      const target = state.snapshot.connected
        ? state.snapshot.agents.find((a) => a.id === agent)
        : null;
      if (target?.viewport) deps.onFollow(target.viewport);
      else if (!target) state.following = null;
      notify();
    },
    openSession(agent: AgentId) {
      const a = state.snapshot.agents.find((a) => a.id === agent);
      if (a) deps.onOpenSession?.(a.session);
    },
    rejected(frame: NodeId, agent: AgentId) {
      deps.onRejected?.(frame, agent);
    },
    async action(action: PresenceAction) {
      if (state.pending || !state.snapshot.connected) return;
      state = { ...state, pending: true, error: null };
      notify();
      try {
        await deps.feed.action(action);
      } catch (e) {
        state = { ...state, error: e instanceof Error ? e.message : String(e) };
      } finally {
        state = { ...state, pending: false };
        notify();
      }
    },
    subscribe(l: () => void) {
      listeners.add(l);
      return { dispose: () => void listeners.delete(l) };
    },
    dispose() {
      disposed = true;
      sub.dispose();
      commits?.dispose();
      listeners.clear();
    },
  };
}
export type PresenceController = ReturnType<typeof createPresenceController>;
export class FakePresenceFeed implements PresenceFeed {
  readonly requests: PresenceAction[] = [];
  fail: string | null = null;
  private listeners = new Set<(s: PresenceSnapshot) => void>();
  constructor(private snapshot: PresenceSnapshot = EMPTY) {}
  async read() {
    if (this.fail) throw Error(this.fail);
    return cloneJson(this.snapshot);
  }
  subscribe(l: (s: PresenceSnapshot) => void) {
    this.listeners.add(l);
    return { dispose: () => void this.listeners.delete(l) };
  }
  emit(s: PresenceSnapshot) {
    this.snapshot = cloneJson(s);
    for (const l of this.listeners) l(cloneJson(s));
  }
  async action(a: PresenceAction) {
    if (this.fail) throw Error(this.fail);
    this.requests.push(cloneJson(a));
  }
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
