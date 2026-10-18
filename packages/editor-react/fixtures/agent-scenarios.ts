// TEST DATA. The Juniper sessions of design screens C03, C06 and C09, as the daemon's session log would hold them,
// and the installed-engine transcripts of fixtures/agent-panel/ (Prompts 36–37) for replay.
import {
  FakeAgentFeed,
  type FeedUpdate,
  type SessionEvent,
  type SessionInfo,
  type StepDetail,
  type TranscriptFixture,
} from "@vellum/editor-core";
import claudeCode from "../../../fixtures/agent-panel/claude-code.json";
import codex from "../../../fixtures/agent-panel/codex.json";
import type { ActorView } from "../src/chrome/ActorMarker";

export const AGENTS = {
  teal: "01J00000000000000000000A01",
  saffron: "01J00000000000000000000A03",
  rose: "01J00000000000000000000A04",
  green: "01J00000000000000000000A05",
  claude: "01J00000000000000000000A09",
  codex: "01J00000000000000000000A02",
} as const;
export const AGENT_ACTORS: Record<string, ActorView> = {
  [AGENTS.teal]: { id: AGENTS.teal, kind: "agent", name: "Teal", hue: "teal" },
  [AGENTS.saffron]: { id: AGENTS.saffron, kind: "agent", name: "Saffron", hue: "saffron" },
  [AGENTS.rose]: { id: AGENTS.rose, kind: "agent", name: "Rose", hue: "rose" },
  [AGENTS.green]: { id: AGENTS.green, kind: "agent", name: "Green", hue: "green" },
  [AGENTS.claude]: { id: AGENTS.claude, kind: "agent", name: "Claude Code", hue: "magenta" },
  [AGENTS.codex]: { id: AGENTS.codex, kind: "agent", name: "Saffron", hue: "saffron" },
};
export const SESSIONS = {
  onboarding: "01J00000000000000000000S01",
  claude: "01J00000000000000000000S02",
  paywall: "01J00000000000000000000S03",
  tokens: "01J00000000000000000000S04",
} as const;
const TURN = "01J00000000000000000000T01";
const DEVICE = "01J00000000000000000000D01";
/** 2026-10-18 10:20 UTC; the scenarios' clock. */
export const AGENT_NOW = Date.UTC(2026, 9, 18, 10, 20);
const MODEL = "deepseek/deepseek-v4.1-flash";
/** A picture the cache does not have: the panel draws its missing-picture pattern, as the design's export does. */
const WELCOME_PNG = "c".repeat(64);
const SHOT_PNG = "d".repeat(64);

/** Layer names the panel looks up for a running step's line. */
export const AGENT_LAYER_NAMES: Record<string, string> = {
  welcome: "Welcome",
  step1: "Step 1",
  step2: "Step 2",
  step3: "Step 3",
};
export type AgentScenario = "c03" | "c06" | "c06e" | "c09" | "claude-code" | "codex" | "stream";

let seq = 0;
const ev = (
  session: string,
  body: SessionEvent["body"],
  at: number,
  actor: SessionEvent["origin"]["actor"] = { kind: "agent", agent: AGENTS.teal },
  turn: string | null = TURN,
): SessionEvent => ({ seq: ++seq, at, origin: { actor, session, turn }, body });
const person = { kind: "person", device: DEVICE } as const;
const step = (
  session: string,
  tool: string,
  at: number,
  seconds: number,
  detail: StepDetail,
  extra: { arguments?: string; thumbnail?: string; outcome?: "done" | "failed" } = {},
  actor?: SessionEvent["origin"]["actor"],
  turn?: string | null,
) =>
  ev(
    session,
    {
      type: "step",
      tool,
      arguments: extra.arguments ?? "{}",
      result: '{"ok":true}',
      startedAt: at - seconds * 1000,
      durationMs: seconds * 1000,
      outcome: extra.outcome ?? "done",
      detail: { tool, outcome: extra.outcome ?? "done", ...detail },
      ...(extra.thumbnail ? { thumbnail: extra.thumbnail } : {}),
    },
    at,
    actor,
    turn,
  );
const counts = (created: number, changed = 0) => ({ created, changed, removed: 0, variables: 0 });
const frame = (id: string, name: string) => ({ id, name, type: "frame" });
const start = (session: string, at: number): SessionEvent[] => [
  ev(
    session,
    {
      type: "prompt",
      text: "Design the onboarding screens in @DESIGN.md using Starter Minimal.",
      context: ["DESIGN.md", "Starter Minimal"],
      images: [],
    },
    at,
    person,
  ),
  ev(
    session,
    { type: "turn", turn: TURN, from: "queued", input: "start", to: "planning" },
    at + 100,
    person,
  ),
  ev(
    session,
    { type: "turn", turn: TURN, from: "planning", input: "no_plan_needed", to: "running" },
    at + 200,
  ),
];

const session = (id: string, extra: Partial<SessionInfo>): SessionInfo => ({
  id,
  kind: { type: "engine", engine: "builtin" },
  agent: AGENTS.teal,
  title: "Onboarding screens",
  createdAt: AGENT_NOW - 30 * 60_000,
  updatedAt: AGENT_NOW - 1000,
  state: "running",
  agents: 1,
  model: MODEL,
  ...extra,
});
const switcherSessions = (): SessionInfo[] => [
  session(SESSIONS.onboarding, { agents: 4 }),
  session(SESSIONS.claude, {
    kind: { type: "external", client: "Claude Code" },
    agent: AGENTS.claude,
    title: "Claude Code",
    createdAt: AGENT_NOW - 8 * 60_000,
    updatedAt: AGENT_NOW - 2000,
    state: null,
    connected: true,
  }),
  session(SESSIONS.paywall, {
    agent: AGENTS.saffron,
    title: "Paywall variants",
    waiting: "choice",
    updatedAt: AGENT_NOW - 5000,
  }),
  session(SESSIONS.tokens, {
    agent: AGENTS.green,
    title: "Token cleanup",
    state: "done",
    updatedAt: Date.UTC(2026, 9, 18, 10, 12),
  }),
];

function building(): { events: SessionEvent[]; live: FeedUpdate[] } {
  const s = SESSIONS.onboarding;
  const t0 = AGENT_NOW - 40_000;
  const events = [
    ...start(s, t0),
    step(s, "read_file", t0 + 2000, 1, { verb: "read" }, { arguments: '{"path":"DESIGN.md"}' }),
    step(s, "read_skill", t0 + 4000, 2, { verb: "read" }, { arguments: '{"path":"guidelines"}' }),
    step(s, "claim_frame", t0 + 5000, 1, { verb: "claimed", target: frame("welcome", "Welcome") }),
    step(
      s,
      "insert_nodes",
      t0 + 24_000,
      18,
      {
        verb: "designed",
        target: frame("welcome", "Welcome"),
        counts: counts(28),
        thumbnail: { node: "welcome", scale: 0.25 },
      },
      { thumbnail: WELCOME_PNG },
    ),
  ];
  const live = (item: Extract<FeedUpdate, { type: "live" }>["item"]): FeedUpdate => ({
    type: "live",
    session: s,
    turn: TURN,
    item,
  });
  return {
    events: events.slice(0, 3),
    live: [
      live({ type: "text", text: "I've built Welcome. I'm building Step 1, then Steps 2 and 3." }),
      ...events.slice(3).map((event): FeedUpdate => ({ type: "event", session: s, event })),
      live({
        type: "toolStarted",
        call: "c5",
        name: "insert_nodes",
        arguments: { parent: "step1" },
      }),
    ],
  };
}

function external(): SessionEvent[] {
  const s = SESSIONS.claude;
  const claude = { kind: "agent", agent: AGENTS.claude } as const;
  const t0 = AGENT_NOW - 60_000;
  return [
    step(
      s,
      "read_file",
      t0,
      1,
      { verb: "read" },
      { arguments: '{"path":"DESIGN.md"}' },
      claude,
      null,
    ),
    step(
      s,
      "insert_nodes",
      t0 + 17_000,
      15,
      {
        verb: "designed",
        target: frame("step2", "Step 2"),
        counts: counts(28),
        thumbnail: { node: "step2", scale: 0.25 },
      },
      { thumbnail: WELCOME_PNG },
      claude,
      null,
    ),
    step(s, "screenshot", t0 + 18_000, 1, { verb: "checked_screenshot" }, {}, claude, null),
  ];
}

function details(): SessionEvent[] {
  const s = SESSIONS.onboarding;
  const t0 = AGENT_NOW - 120_000;
  const step2 = frame("step2", "Step 2");
  return [
    ...start(s, t0),
    step(s, "insert_nodes", t0 + 40_000, 30, {
      verb: "designed",
      target: step2,
      counts: counts(31),
    }),
    step(s, "update_nodes", t0 + 50_000, 5, {
      verb: "updated",
      target: frame("step3", "Step 3"),
      counts: counts(0, 16),
    }),
    step(
      s,
      "screenshot",
      t0 + 55_000,
      3,
      {
        verb: "checked_screenshot",
        target: step2,
        message: "Fixed: CTA text overflowed at 375 px",
      },
      { thumbnail: SHOT_PNG },
    ),
    step(
      s,
      "fill_image",
      t0 + 60_000,
      4,
      { verb: "added_asset", target: step2, error: "The image service did not answer in time." },
      { outcome: "failed" },
    ),
    ev(
      s,
      {
        type: "usage",
        model: MODEL,
        tokens: { input: 30_000, output: 2_100, cacheRead: 5_900, cacheWrite: 0 },
      },
      t0 + 70_000,
    ),
    ev(
      s,
      {
        type: "messages",
        messages: [
          {
            role: "assistant",
            parts: [
              { text: "I've built Step 2. The chart uses placeholder data." },
              ...["insert_nodes", "update_nodes", "screenshot", "fill_image"].map((name, i) => ({
                toolCall: { id: `c${i}`, name, arguments: {}, metadata: null },
              })),
            ],
          },
        ],
      },
      t0 + 71_000,
    ),
    ev(s, { type: "turn", turn: TURN, from: "running", input: "done", to: "done" }, t0 + 72_000),
  ];
}

/** The feed, its scripted updates (sent after the panel loads) and what the panel opens with. */
export function agentScenario(name: AgentScenario): {
  feed: FakeAgentFeed;
  updates: FeedUpdate[];
  current?: string;
  expanded?: string[];
  /** Time that passes after the updates, so a working step has run a while (C03: 6 s). */
  elapse?: number;
} {
  seq = 0;
  if (name === "claude-code" || name === "codex") {
    const f = (name === "codex" ? codex : claudeCode) as unknown as TranscriptFixture;
    const feed = FakeAgentFeed.fromTranscript(f, { upTo: 3 });
    const turn = f.events[0]?.origin.turn as string;
    return {
      feed,
      updates: [
        ...f.live.map((item): FeedUpdate => ({ type: "live", session: f.session.id, turn, item })),
        ...f.events
          .slice(3)
          .map((event): FeedUpdate => ({ type: "event", session: f.session.id, event })),
      ],
    };
  }
  if (name === "c06" || name === "c06e") {
    const b = building();
    return {
      feed: new FakeAgentFeed(switcherSessions(), {
        [SESSIONS.onboarding]: b.events,
        [SESSIONS.claude]: external(),
      }),
      updates: name === "c06" ? b.live : [],
      current: name === "c06e" ? SESSIONS.claude : SESSIONS.onboarding,
    };
  }
  if (name === "c09") {
    const events = details();
    return {
      feed: new FakeAgentFeed([session(SESSIONS.onboarding, { state: "done", agents: 1 })], {
        [SESSIONS.onboarding]: events,
      }),
      updates: [],
      expanded: [
        String(events.find((e) => e.body.type === "step" && e.body.tool === "screenshot")?.seq),
      ],
    };
  }
  const b = building();
  return {
    feed: new FakeAgentFeed([session(SESSIONS.onboarding, {})], {
      [SESSIONS.onboarding]: b.events,
    }),
    updates: b.live,
    elapse: name === "c03" ? 6000 : 0,
  };
}
