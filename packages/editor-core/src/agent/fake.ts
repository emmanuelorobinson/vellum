/** A scriptable AgentFeed. It never runs a model, a daemon or a timer: tests emit what the daemon would. */
import type { SessionId, TurnId } from "@vellum/types-ts";
import type { AgentFeed, FeedRequest, FeedUpdate, LiveEvent } from "./feed";
import type { SessionEvent } from "./log";
import type { SessionInfo } from "./sessions";

/** fixtures/agent-panel/<engine>.json, written by crates/vellum/tests/agent_panel_transcripts.rs. */
export interface TranscriptFixture {
  engine: string;
  transcript: string;
  session: {
    id: SessionId;
    kind: SessionInfo["kind"];
    agent: string;
    title: string;
    createdAt: number;
  };
  live: LiveEvent[];
  events: SessionEvent[];
}

export class FakeAgentFeed implements AgentFeed {
  readonly requests: FeedRequest[] = [];
  readonly created: SessionId[] = [];
  /** When set, every request fails with this message. */
  fail: string | null = null;
  private listeners = new Set<(u: FeedUpdate) => void>();
  private list: SessionInfo[];
  private logs: Record<string, SessionEvent[]>;
  private images: Record<string, string>;
  private counter = 0;

  constructor(
    sessions: SessionInfo[],
    logs: Record<string, SessionEvent[]> = {},
    images: Record<string, string> = {},
  ) {
    this.list = sessions.map((s) => ({ ...s }));
    this.logs = Object.fromEntries(Object.entries(logs).map(([k, v]) => [k, [...v]]));
    this.images = images;
  }

  /** A feed holding one transcript's session, its log cut after `upTo` events. */
  static fromTranscript(f: TranscriptFixture, { upTo }: { upTo?: number } = {}): FakeAgentFeed {
    const events = f.events.slice(0, upTo ?? f.events.length);
    return new FakeAgentFeed(
      [
        {
          id: f.session.id,
          kind: f.session.kind,
          agent: f.session.agent,
          title: f.session.title,
          createdAt: f.session.createdAt,
          updatedAt: events[events.length - 1]?.at ?? f.session.createdAt,
          state: null,
          agents: 1,
        },
      ],
      { [f.session.id]: events },
    );
  }

  private check() {
    if (this.fail) return Promise.reject(Error(this.fail));
    return null;
  }
  async sessions() {
    await this.check();
    return this.list.map((s) => ({ ...s }));
  }
  async events(session: SessionId) {
    await this.check();
    return [...(this.logs[session] ?? [])];
  }
  subscribe(listener: (u: FeedUpdate) => void) {
    this.listeners.add(listener);
    return { dispose: () => void this.listeners.delete(listener) };
  }
  /** What the daemon would send. Events are kept, so a later `events()` returns them too. */
  emit(update: FeedUpdate) {
    if (update.type === "event") {
      const log = this.logs[update.session] ?? [];
      if (!log.some((e) => e.seq === update.event.seq)) log.push(update.event);
      this.logs[update.session] = log;
    }
    if (update.type === "sessions") this.list = update.sessions.map((s) => ({ ...s }));
    for (const l of [...this.listeners]) l(update);
  }
  async create() {
    await this.check();
    const id = `01J0000000000000000000N${String(++this.counter).padStart(3, "0")}` as SessionId;
    const s: SessionInfo = {
      id,
      kind: { type: "engine", engine: "builtin" },
      agent: this.list[0]?.agent ?? "01J00000000000000000000A01",
      title: "New session",
      createdAt: 0,
      updatedAt: Number.MAX_SAFE_INTEGER,
      state: null,
      agents: 1,
    };
    this.created.push(id);
    this.list.push(s);
    this.logs[id] = [];
    return { ...s };
  }
  async remove(session: SessionId) {
    await this.check();
    this.requests.push({ type: "remove", session });
    this.list = this.list.filter((s) => s.id !== session);
    delete this.logs[session];
  }
  async undoTurn(session: SessionId, turn: TurnId | null) {
    await this.check();
    this.requests.push({ type: "undoTurn", session, turn });
  }
  async retry(session: SessionId, turn: TurnId) {
    await this.check();
    this.requests.push({ type: "retry", session, turn });
  }
  async retryStep(session: SessionId, turn: TurnId, seq: number) {
    await this.check();
    this.requests.push({ type: "retryStep", session, turn, seq });
  }
  async stop(session: SessionId) {
    await this.check();
    this.requests.push({ type: "stop", session });
  }
  image(hash: string) {
    return this.images[hash] ?? null;
  }
}
