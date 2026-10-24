/**
 * Presence for a host whose agents run outside the editor (the desktop's installed engines): what the editor itself
 * can see. A running turn says an agent is here; the nodes the document gained or changed while it ran say where it
 * is. The cursor walks those nodes, one every `STEP_MS`, so it moves like a hand rather than jumping to the end.
 */
import type { Box, EngineClient } from "@vellum/engine-client";
import type { AgentId, NodeId, PageId, SessionId } from "@vellum/types-ts";
import type { AgentController } from "./controller";
import type {
  FrameLease,
  PresenceAction,
  PresenceFeed,
  PresenceSnapshot,
  PresentAgent,
} from "./presence";
import type { TurnItem } from "./turns";

/** The agent hues, in the order they are handed out (design tokens `agent/*`). */
export const AGENT_HUES = ["teal", "orange", "magenta", "green", "saffron", "rose", "lime", "sand"];
/** A moment between cursor stops; the canvas glides between them (DESIGN.md: cursors glide over 100 ms). */
export const STEP_MS = 160;
const DIFF_DELAY_MS = 90;
const MAX_NODES = 4000;
const MAX_QUEUE = 14;
/** How long a lease stays after its agent's last change, so a finished frame does not vanish mid-glance. */
const LEASE_LINGER_MS = 2500;

export interface LivePresenceActor {
  id: string;
  kind: "agent";
  name: string;
  hue: string;
  state: string;
}

export interface LivePresenceDeps {
  agent: Pick<AgentController, "turns" | "subscribe"> & { state: { current: string | null } };
  client: Pick<EngineClient, "scene" | "subscribe">;
  now(): number;
  /** The page the editor shows: leases and cursors belong to it. */
  page(): PageId;
  /** What the engine that runs the next turn is called ("Codex", "Claude Code"). */
  engineName(): string;
  /** The engine's id, to give each engine its own hue. */
  engineId(): string;
  /** The daemon holds this file: without it nothing is shown. */
  connected(): boolean;
  paused?(): boolean;
  onAction?(action: PresenceAction): void | Promise<void>;
  timers?: {
    set(fn: () => void, ms: number): unknown;
    clear(handle: unknown): void;
  };
}

interface Seen {
  print: string;
  parent: string | null;
}
type Seens = Map<string, Seen>;

/** One fingerprint per node: everything but its children, which are nodes of their own. */
function snapshotScene(client: LivePresenceDeps["client"]): Seens | null {
  const scene = client.scene;
  const out: Seens = new Map();
  const visit = (id: string): boolean => {
    if (out.size > MAX_NODES) return false;
    const item = scene.get(id);
    if (!item) return true;
    const { children: _children, ...own } = item.node as unknown as Record<string, unknown>;
    out.set(id, { print: JSON.stringify(own), parent: item.parent });
    return item.children.every(visit);
  };
  return scene.roots().every(visit) ? out : null;
}

const area = (b: Box) => b.width * b.height;

export function createLivePresence(deps: LivePresenceDeps) {
  const timers = deps.timers ?? {
    set: (fn: () => void, ms: number) => setTimeout(fn, ms),
    clear: (h: unknown) => clearTimeout(h as ReturnType<typeof setTimeout>),
  };
  const listeners = new Set<(snapshot: PresenceSnapshot) => void>();
  let running: { id: string; session: string; state: PresentAgent["state"]; task?: string }[] = [];
  let baseline: Seens | null = null;
  let cursor: { x: number; y: number } | undefined;
  let queue: { x: number; y: number }[] = [];
  const touched = new Map<string, { agent: string; changes: number; at: number }>();
  let walker: unknown;
  let diffTimer: unknown;
  let disposed = false;
  const known = new Map<string, LivePresenceActor>();

  const hueOf = (engine: string) => {
    let h = 0;
    for (const c of engine) h = (h * 31 + c.charCodeAt(0)) >>> 0;
    return AGENT_HUES[h % AGENT_HUES.length] as string;
  };
  const taskOf = (items: TurnItem[]): string | undefined => {
    const last = items.at(-1);
    const step = last?.type === "step" ? last.step : last?.type === "group" ? last.steps.at(-1) : null;
    return step && step.state === "working" ? step.line : undefined;
  };
  const readRunning = () => {
    const out: typeof running = [];
    for (const turn of deps.agent.turns()) {
      if (turn.external || !turn.id) continue;
      const state: PresentAgent["state"] | null =
        turn.state === "running" || turn.state === "planning"
          ? "working"
          : turn.state === "awaiting_permission"
            ? "blocked_by_permission"
            : null;
      const id = turn.agents[0];
      if (!state || !id) continue;
      const task = taskOf(turn.items);
      out.push({
        id,
        session: deps.agent.state.current ?? "",
        state,
        ...(task ? { task } : {}),
      });
    }
    return out;
  };

  const snapshot = (): PresenceSnapshot => {
    const page = deps.page();
    const name = deps.engineName();
    const hue = hueOf(deps.engineId());
    const agents: PresentAgent[] = running.map((a) => {
      known.set(a.id, { id: a.id, kind: "agent", name, hue, state: a.state });
      return {
        id: a.id as AgentId,
        name,
        hue,
        session: a.session as SessionId,
        state: a.state,
        engine: deps.engineId(),
        ...(a.task ? { task: a.task } : {}),
        // The cursor belongs to the agent that is working; with several, the latest takes it.
        ...(cursor && a === running.at(-1) ? { cursor } : {}),
      };
    });
    const now = deps.now();
    const leases: FrameLease[] = [];
    for (const [frame, t] of touched) {
      const item = deps.client.scene.get(frame);
      if (!item) continue;
      const live = running.some((a) => a.id === t.agent);
      if (!live && now - t.at > LEASE_LINGER_MS) continue;
      leases.push({
        frame: frame as NodeId,
        agent: t.agent as AgentId,
        page,
        bounds: item.bounds,
        changes: t.changes,
        rejected: 0,
        editing: false,
        idle: !live,
      });
    }
    return {
      connected: deps.connected(),
      paused: deps.paused?.() ?? false,
      agents,
      leases,
      highlights: [],
    };
  };
  const emit = () => {
    if (disposed) return;
    const s = snapshot();
    for (const l of [...listeners]) l(s);
  };

  const topLevel = (seens: Seens, id: string): string => {
    let at = id;
    for (let parent = seens.get(at)?.parent; parent; parent = seens.get(at)?.parent) at = parent;
    return at;
  };
  const stopWalking = () => {
    if (walker !== undefined) timers.clear(walker);
    walker = undefined;
  };
  const walk = () => {
    walker = undefined;
    const next = queue.shift();
    if (next) cursor = next;
    emit();
    if (queue.length) walker = timers.set(walk, STEP_MS);
  };
  const diff = () => {
    diffTimer = undefined;
    if (disposed || !baseline || !running.length) return;
    const now = deps.now();
    const seens = snapshotScene(deps.client);
    if (!seens) return;
    const changed: string[] = [];
    for (const [id, seen] of seens)
      if (baseline.get(id)?.print !== seen.print) changed.push(id);
    baseline = seens;
    if (!changed.length) return;
    const agent = (running.at(-1) as (typeof running)[number]).id;
    const stops: { x: number; y: number; size: number }[] = [];
    for (const id of changed) {
      const frame = topLevel(seens, id);
      const t = touched.get(frame);
      touched.set(frame, { agent, changes: (t?.changes ?? 0) + 1, at: now });
      const box = deps.client.scene.get(id)?.bounds;
      if (box) stops.push({ x: box.x + box.width / 2, y: box.y + box.height / 2, size: area(box) });
    }
    // Largest first reads as a hand blocking in the layout, then filling it.
    stops.sort((a, b) => b.size - a.size);
    const stride = Math.max(1, Math.ceil(stops.length / MAX_QUEUE));
    queue = stops.filter((_, i) => i % stride === 0).map(({ x, y }) => ({ x, y }));
    if (walker === undefined) walk();
  };
  const scheduleDiff = () => {
    if (diffTimer === undefined && baseline) diffTimer = timers.set(diff, DIFF_DELAY_MS);
  };

  const onTurns = () => {
    const before = running.length;
    running = readRunning();
    if (running.length && !baseline) baseline = snapshotScene(deps.client);
    if (!running.length && before) {
      // The turn is over: the cursor leaves, the leases linger a moment and then clear themselves.
      baseline = null;
      cursor = undefined;
      queue = [];
      stopWalking();
      for (const [frame, t] of touched) touched.set(frame, { ...t, at: deps.now() });
      timers.set(() => {
        if (!running.length) touched.clear();
        emit();
      }, LEASE_LINGER_MS + 50);
    }
    emit();
  };
  const turns = deps.agent.subscribe(onTurns);
  const doc = deps.client.subscribe(scheduleDiff);
  running = readRunning();
  if (running.length) baseline = snapshotScene(deps.client);

  const feed: PresenceFeed = {
    async read() {
      return snapshot();
    },
    subscribe(listener) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
    async action(action) {
      await deps.onAction?.(action);
    },
  };
  return {
    feed,
    /** The agents seen so far, for the Agent tab's markers and the canvas to agree on a name and hue. */
    actors(): Record<string, LivePresenceActor> {
      return Object.fromEntries(known);
    },
    dispose() {
      disposed = true;
      stopWalking();
      if (diffTimer !== undefined) timers.clear(diffTimer);
      turns.dispose();
      doc.dispose();
      listeners.clear();
    },
  };
}
export type LivePresence = ReturnType<typeof createLivePresence>;
