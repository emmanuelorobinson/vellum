/**
 * The Agent tab's controller: which session is open, its log and live stream, the switcher, expanded rows and the
 * Undo-this-turn confirmation. Every action goes to the AgentFeed; nothing here changes a document.
 */
import type { AgentId, NodeId, SessionId, TurnId } from "@vellum/types-ts";
import type { AgentFeed, FeedUpdate } from "./feed";
import type { SessionEvent } from "./log";
import { groupSessions, type SessionInfo } from "./sessions";
import { targetOf, toolName } from "./steps";
import { buildTurns, type LiveItem, type TurnView } from "./turns";

export interface AgentControllerDeps {
  feed: AgentFeed;
  now(): number;
  timeZone?: string;
  /** A thumbnail was clicked: zoom the canvas to that frame and select it. */
  onZoomTo?(node: NodeId): void;
  /** A layer's name, for the line of a step still running ("Designing Step 1"). */
  nameOf?(node: NodeId): string | null;
  /** Review changes: open History filtered to this turn. */
  onReviewChanges?(session: SessionId, turn: TurnId | null): void;
}

export interface AgentState {
  sessions: SessionInfo[];
  current: SessionId | null;
  events: SessionEvent[];
  /** The live stream per turn, for the open session. */
  live: Record<string, LiveItem[]>;
  switcher: boolean;
  expanded: ReadonlySet<string>;
  /** The turn whose Undo this turn confirmation is open (`"external"` for an external agent's steps). */
  confirmUndo: string | null;
  error: string | null;
  loading: boolean;
}

export function createAgentController(deps: AgentControllerDeps) {
  const { feed } = deps;
  let state: AgentState = {
    sessions: [],
    current: null,
    events: [],
    live: {},
    switcher: false,
    expanded: new Set(),
    confirmUndo: null,
    error: null,
    loading: false,
  };
  // A slower answer for a session the person has already left is dropped.
  let generation = 0;
  let cache: { key: unknown[]; turns: TurnView[] } | null = null;
  const listeners = new Set<() => void>();
  const change = (patch: Partial<AgentState>) => {
    state = { ...state, ...patch };
    for (const l of [...listeners]) l();
  };
  const attempt = async (work: () => Promise<void>) => {
    try {
      await work();
      if (state.error) change({ error: null });
    } catch (e) {
      change({ error: e instanceof Error ? e.message : String(e) });
    }
  };
  const info = () => state.sessions.find((s) => s.id === state.current) ?? null;

  const onUpdate = (u: FeedUpdate) => {
    if (u.type === "sessions") {
      change({ sessions: u.sessions });
      return;
    }
    // Questions, usage, engines and settings are the composer's.
    if (u.type !== "event" && u.type !== "live") return;
    if (u.session !== state.current) return;
    if (u.type === "event") {
      if (state.events.some((e) => e.seq === u.event.seq)) return;
      const turn = u.event.origin.turn;
      let live = state.live;
      if (u.event.body.type === "step" && turn && live[turn]) {
        const items = [...(live[turn] as LiveItem[])];
        const tool = u.event.body.tool;
        const at = items.findIndex((i) => i.type === "working" && toolName(i.tool) === tool);
        // The step takes the place of its own call's working row; another tool's step goes after.
        if (at >= 0) items[at] = { type: "step", seq: u.event.seq };
        else items.push({ type: "step", seq: u.event.seq });
        live = { ...live, [turn]: items };
      }
      change({ events: [...state.events, u.event], live });
      return;
    }
    const items = [...(state.live[u.turn] ?? [])];
    if (u.item.type === "text") items.push({ type: "text", text: u.item.text });
    else if (u.item.type === "toolStarted") {
      const node = targetOf(u.item.arguments);
      items.push({
        type: "working",
        call: u.item.call,
        tool: u.item.name,
        startedAt: deps.now(),
        target: node ? (deps.nameOf?.(node) ?? null) : null,
      });
    } else return;
    change({ live: { ...state.live, [u.turn]: items } });
  };
  const subscription = feed.subscribe(onUpdate);

  const open = async (id: SessionId | null) => {
    const mine = ++generation;
    change({
      current: id,
      events: [],
      live: {},
      switcher: false,
      confirmUndo: null,
      loading: !!id,
    });
    if (!id) return;
    await attempt(async () => {
      const events = await feed.events(id);
      if (mine === generation) change({ events, loading: false });
    });
  };

  return {
    get state() {
      return state;
    },
    async load() {
      await attempt(async () => {
        const sessions = await feed.sessions();
        change({ sessions });
        const newest = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt)[0];
        if (!state.current && newest) await open(newest.id);
      });
    },
    session: info,
    groups: () => groupSessions(state.sessions),
    turns(): TurnView[] {
      const s = info();
      const now = deps.now();
      // Rebuilt only when something it reads changed; a working step's clock needs `now` too.
      const key = [state.events, state.live, s, now];
      if (cache?.key.every((k, i) => k === key[i])) return cache.turns;
      const turns = buildTurns(state.events, {
        live: state.live,
        now,
        external: s?.kind.type === "external",
        model: s?.model ?? null,
      });
      cache = { key, turns };
      return turns;
    },
    toggleSwitcher(open = !state.switcher) {
      change({ switcher: open });
    },
    select: (id: SessionId) => open(id),
    async newSession() {
      await attempt(async () => {
        const s = await feed.create();
        change({ sessions: [...state.sessions.filter((x) => x.id !== s.id), s] });
        await open(s.id);
      });
    },
    async deleteSession(id: SessionId) {
      await attempt(async () => {
        await feed.remove(id);
        const sessions = state.sessions.filter((s) => s.id !== id);
        change({ sessions });
        if (state.current === id)
          await open([...sessions].sort((a, b) => b.updatedAt - a.updatedAt)[0]?.id ?? null);
      });
    },
    expanded: (key: string) => state.expanded.has(key),
    toggle(key: string) {
      const expanded = new Set(state.expanded);
      if (!expanded.delete(key)) expanded.add(key);
      change({ expanded });
    },
    askUndo(turn: TurnId | null) {
      change({ confirmUndo: turn ?? "external" });
    },
    cancelUndo() {
      change({ confirmUndo: null });
    },
    /** What the confirmation says: "Removes {n} changes from {agent}'s turn." */
    undoPrompt(): { turn: TurnId | null; changes: number; agent: AgentId | null } | null {
      const key = state.confirmUndo;
      if (!key) return null;
      const t = this.turns().find((x) => x.key === key);
      if (!t) return null;
      return { turn: t.id, changes: t.footer?.changes ?? 0, agent: t.agents[0] ?? null };
    },
    async confirmUndo() {
      const prompt = this.undoPrompt();
      const session = state.current;
      change({ confirmUndo: null });
      if (prompt && session) await attempt(() => feed.undoTurn(session, prompt.turn));
    },
    async retry(turn: TurnId) {
      const session = state.current;
      if (session) await attempt(() => feed.retry(session, turn));
    },
    async retryStep(turn: TurnId, seq: number) {
      const session = state.current;
      if (session) await attempt(() => feed.retryStep(session, turn, seq));
    },
    async stop() {
      const session = state.current;
      if (session) await attempt(() => feed.stop(session));
    },
    reviewChanges(turn: TurnId | null) {
      if (state.current) deps.onReviewChanges?.(state.current, turn);
    },
    zoomTo(node: NodeId) {
      deps.onZoomTo?.(node);
    },
    image: (hash: string | null) => (hash ? feed.image(hash) : null),
    timeOptions: () => ({ now: deps.now(), ...(deps.timeZone ? { timeZone: deps.timeZone } : {}) }),
    clearError() {
      change({ error: null });
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
    dispose() {
      generation++;
      subscription.dispose();
      listeners.clear();
    },
  };
}
export type AgentController = ReturnType<typeof createAgentController>;
