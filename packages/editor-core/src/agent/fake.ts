/** A scriptable AgentFeed. It never runs a model, a daemon or a timer: tests emit what the daemon would. */
import type { AgentId, SessionId, TurnId } from "@vellum/types-ts";
import type { EngineCatalog, ProviderSetup, ProviderStatusName } from "./engines";
import type {
  AgentFeed,
  FeedRequest,
  FeedUpdate,
  LiveEvent,
  PromptRequest,
  SkillInfo,
} from "./feed";
import type { SessionEvent } from "./log";
import type { AgentPlan, PlanBuild } from "./orchestration";
import type { PermissionAnswer, PermissionQuestion } from "./permissions";
import type { SessionInfo } from "./sessions";
import type { SessionUsage } from "./usage";

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
  /** Every request the composer sent, with its session. */
  readonly sent: { session: SessionId; request: PromptRequest; turn: TurnId }[] = [];
  /** Keys as the keychain would hold them. Never in `requests`, never sent back. */
  readonly keys: Record<string, string> = {};
  /** When set, every request fails with this message. */
  fail: string | null = null;
  skillList: SkillInfo[] = [];
  /** What the next Add skill… picks; `null` is a cancelled folder picker. */
  nextSkill: SkillInfo | null = null;
  fileList: string[] = [];
  catalog: EngineCatalog = {
    builtin: [
      {
        id: "openrouter",
        name: "OpenRouter",
        status: "api_key_set",
        models: [
          {
            id: "deepseek/deepseek-v4.1-flash",
            label: "deepseek/deepseek-v4.1-flash",
            caps: { vision: true, fast: true, reasoning: false, contextTokens: 1_000_000 },
          },
        ],
      },
      {
        id: "anthropic",
        name: "Anthropic",
        status: "api_key_set",
        models: [
          {
            id: "claude-sonnet",
            label: "Claude Sonnet",
            caps: { vision: true, fast: false, reasoning: true, contextTokens: 1_000_000 },
          },
        ],
      },
    ],
    installed: [
      { id: "claude-code", name: "Claude Code", state: "signed_in", command: "claude" },
      { id: "codex", name: "Codex", state: "signed_in", command: "codex" },
    ],
  };
  /** What a provider's test answers. */
  testResult: ProviderStatusName = "connected";
  skipAll = false;
  usages: Record<string, SessionUsage> = {};
  private pending: Record<string, PermissionQuestion[]> = {};
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
    if (update.type === "question")
      this.pending[update.session] = [
        ...(this.pending[update.session] ?? []).filter((q) => q.id !== update.question.id),
        update.question,
      ];
    if (update.type === "answered")
      this.pending[update.session] = (this.pending[update.session] ?? []).filter(
        (q) => q.id !== update.question,
      );
    if (update.type === "usage") this.usages[update.session] = update.usage;
    if (update.type === "permissionSettings") this.skipAll = update.skipAll;
    if (update.type === "engines") this.catalog = update.catalog;
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
  async send(session: SessionId, request: PromptRequest) {
    await this.check();
    const turn =
      `01J0000000000000000000T${String(this.sent.length + 1).padStart(3, "0")}` as TurnId;
    this.sent.push({
      session,
      request: JSON.parse(JSON.stringify(request)) as PromptRequest,
      turn,
    });
    return { turn };
  }
  async buildPlan(session: SessionId, turn: TurnId, plan: AgentPlan, parallel: PlanBuild) {
    await this.check();
    this.requests.push(cloneJson({ type: "buildPlan", session, turn, plan, parallel }));
  }
  async editRequest(session: SessionId, turn: TurnId, text: string) {
    await this.check();
    this.requests.push({ type: "editRequest", session, turn, text });
  }
  async keepOptions(session: SessionId, turn: TurnId, options: number[]) {
    await this.check();
    this.requests.push({ type: "keepOptions", session, turn, options: [...options] });
  }
  async stopAgent(session: SessionId, agent: AgentId) {
    await this.check();
    this.requests.push({ type: "stopAgent", session, agent });
  }
  async questions(session: SessionId) {
    await this.check();
    return [...(this.pending[session] ?? [])];
  }
  async answer(session: SessionId, question: string, answer: PermissionAnswer) {
    await this.check();
    this.requests.push({ type: "answer", session, question, answer });
  }
  async permissionSettings() {
    await this.check();
    return { skipAll: this.skipAll };
  }
  async setSkipAll(on: boolean) {
    await this.check();
    this.requests.push({ type: "setSkipAll", on });
    this.skipAll = on;
  }
  async usage(session: SessionId) {
    await this.check();
    return this.usages[session] ?? null;
  }
  async skills() {
    await this.check();
    return this.skillList.map((s) => ({ ...s }));
  }
  async addSkill() {
    await this.check();
    const skill = this.nextSkill;
    if (skill) this.skillList.push(skill);
    this.nextSkill = null;
    return skill ? { ...skill } : null;
  }
  async files(query: string, extensions?: readonly string[]) {
    await this.check();
    const q = query.toLowerCase();
    return this.fileList.filter(
      (f) =>
        f.toLowerCase().includes(q) &&
        (!extensions?.length || extensions.some((e) => f.endsWith(e))),
    );
  }
  async engines() {
    await this.check();
    return JSON.parse(JSON.stringify(this.catalog)) as EngineCatalog;
  }
  private setStatus(provider: string, status: ProviderStatusName) {
    const p = this.catalog.builtin.find((x) => x.id === provider);
    if (p) p.status = status;
    else this.catalog.builtin.push({ id: provider, name: provider, status, models: [] });
    return status;
  }
  async saveProvider(setup: ProviderSetup) {
    await this.check();
    // The key is kept as the keychain would keep it; the request record never holds it.
    if (setup.key) this.keys[setup.provider] = setup.key;
    this.requests.push({
      type: "saveProvider",
      provider: setup.provider,
      ...(setup.endpoint ? { endpoint: setup.endpoint } : {}),
      ...(setup.apiStyle ? { apiStyle: setup.apiStyle } : {}),
    });
    return this.setStatus(setup.provider, this.testResult);
  }
  async recheckProvider(provider: string) {
    await this.check();
    this.requests.push({ type: "recheckProvider", provider });
    return this.setStatus(provider, this.testResult);
  }
  async clearKey(provider: string) {
    await this.check();
    this.requests.push({ type: "clearKey", provider });
    delete this.keys[provider];
    return this.setStatus(provider, "configured_externally");
  }
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
