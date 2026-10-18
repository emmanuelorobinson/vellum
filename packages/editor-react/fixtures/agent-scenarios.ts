// TEST DATA. The Juniper sessions of design screens C03, C06 and C09, as the daemon's session log would hold them,
// and the installed-engine transcripts of fixtures/agent-panel/ (Prompts 36–37) for replay.
import {
  FakeAgentFeed,
  type FeedUpdate,
  type ModelCaps,
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
export type AgentScenario =
  | "c01"
  | "c03"
  | "c06"
  | "c06e"
  | "c08"
  | "c09"
  | "c11"
  | "c26"
  | "c27"
  | "claude-code"
  | "codex"
  | "stream";

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

/** What the composer reads from the daemon in every scenario: C11's providers, C26's skills, C27's usage. */
function composerData(feed: FakeAgentFeed): FakeAgentFeed {
  const model = (id: string, label: string, caps: Partial<ModelCaps> = {}) => ({
    id,
    label,
    caps: { vision: false, fast: false, reasoning: false, contextTokens: 128_000, ...caps },
  });
  feed.catalog = {
    builtin: [
      {
        id: "openrouter",
        name: "OpenRouter",
        status: "connected",
        models: [model(MODEL, MODEL, { vision: true, fast: true, contextTokens: 1_000_000 })],
      },
      {
        id: "anthropic",
        name: "Anthropic",
        status: "api_key_set",
        models: [
          model("claude-sonnet", "Claude Sonnet", { vision: true, contextTokens: 1_000_000 }),
        ],
      },
      {
        id: "openai",
        name: "OpenAI",
        status: "api_key_set",
        models: [
          model("reasoning", "Reasoning model", { vision: true, reasoning: true }),
          model("fast", "Fast model", { vision: true, fast: true }),
        ],
      },
      {
        id: "google",
        name: "Google",
        status: "api_key_set",
        models: [
          model("gemini", "Gemini", { vision: true, contextTokens: 1_000_000 }),
          model("gemini-flash", "Gemini Flash", { vision: true, fast: true }),
        ],
      },
      {
        id: "deepseek",
        name: "DeepSeek",
        status: "api_key_set",
        models: [
          model("deepseek", "DeepSeek", { fast: true, reasoning: true }),
          model("deepseek-chat", "DeepSeek Chat", { fast: true }),
        ],
      },
      {
        id: "xai",
        name: "xAI",
        status: "api_key_set",
        models: [
          model("grok", "Grok", { vision: true, reasoning: true }),
          model("grok-fast", "Grok Fast", { fast: true }),
        ],
      },
      {
        id: "local",
        name: "Local",
        status: "configured_externally",
        models: [model("local", "localhost:11434", { local: true })],
      },
    ],
    installed: [
      { id: "claude-code", name: "Claude Code", state: "signed_in", command: "claude" },
      { id: "codex", name: "Codex", state: "signed_in", command: "codex" },
    ],
  };
  feed.skillList = [
    { path: "workflows/design-a-screen.md", name: "Design a screen", group: "builtin" },
    { path: "workflows/build-a-design-system.md", name: "Build a design system", group: "builtin" },
    { path: "workflows/recreate-from-code.md", name: "Recreate from code", group: "builtin" },
    {
      path: "workflows/make-a-chart-with-a-script.md",
      name: "Make a chart with a script",
      group: "builtin",
    },
    {
      path: "design-system/starter-minimal/guidelines.md",
      name: "Starter Minimal guidelines",
      group: "design_system",
    },
    { path: "~/skills/juniper-voice/SKILL.md", name: "Juniper voice", group: "user" },
    { path: "~/skills/a11y/SKILL.md", name: "Accessibility", group: "user" },
  ];
  feed.fileList = ["DESIGN.md", "README.md", "src/tokens.css", "src/components/HabitCard.tsx"];
  for (const id of Object.values(SESSIONS))
    feed.usages[id] = {
      contextTokens: 8100,
      window: 45_000,
      conversation: 4200,
      files: 2800,
      screenshots: 1100,
      estimate: 40_000,
    };
  return feed;
}

/** C27: Teal asks to read DESIGN.md. */
export const PERMISSION_QUESTION = {
  id: "q-design",
  turn: TURN,
  agent: AGENTS.teal,
  engine: "builtin",
  action: { readFile: { path: "~/code/juniper/DESIGN.md" } },
  kind: "read_repo",
  offered: ["allow_once", "allow_for_session", "deny"],
} as const;

/** The composer's host data: bundled design systems and visual styles (C01-style). */
export const COMPOSER_HOST = {
  designSystems: [
    { id: "starter-minimal", name: "Starter Minimal", swatches: ["#18222D", "#DDE5EE", "#F4F7F9"] },
    {
      id: "starter-editorial",
      name: "Starter Editorial",
      swatches: ["#4C332B", "#D6B59B", "#F6EFE8"],
    },
    {
      id: "starter-technical",
      name: "Starter Technical",
      swatches: ["#17394F", "#9CC8DB", "#E6F1F5"],
    },
  ],
  visualStyles: [
    {
      id: "product",
      name: "Product Data Grid",
      font: "Inter",
      palette: ["#18222D", "#DDE5EE", "#F4F7F9"],
    },
    {
      id: "artisan",
      name: "Artisan Editorial",
      font: "Lora",
      palette: ["#4C332B", "#D6B59B", "#F6EFE8"],
    },
    {
      id: "blueprint",
      name: "Blueprint Technical",
      font: "JetBrains Mono",
      palette: ["#17394F", "#9CC8DB", "#E6F1F5"],
    },
  ],
};

function simpleTurn(s: string, text: string, context: string[], reply: string, done: boolean) {
  const t0 = AGENT_NOW - 60_000;
  return [
    ev(s, { type: "prompt", text, context, images: [] }, t0, person),
    ev(
      s,
      { type: "turn", turn: TURN, from: "queued", input: "start", to: "planning" },
      t0 + 100,
      person,
    ),
    ev(
      s,
      { type: "turn", turn: TURN, from: "planning", input: "no_plan_needed", to: "running" },
      t0 + 200,
    ),
    ...(done
      ? [
          ev(
            s,
            { type: "messages", messages: [{ role: "assistant", parts: [{ text: reply }] }] },
            t0 + 1200,
          ),
          ev(
            s,
            { type: "turn", turn: TURN, from: "running", input: "done", to: "done" },
            t0 + 1300,
          ),
        ]
      : []),
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
  const scenario = baseScenario(name);
  composerData(scenario.feed);
  return scenario;
}

function baseScenario(name: AgentScenario): ReturnType<typeof agentScenario> {
  if (name === "c01" || name === "c26")
    return {
      feed: new FakeAgentFeed(
        [session(SESSIONS.onboarding, { title: "New session", state: null, model: MODEL })],
        { [SESSIONS.onboarding]: [] },
      ),
      updates: [],
    };
  if (name === "c08" || name === "c11")
    return {
      feed: new FakeAgentFeed(
        [
          session(SESSIONS.onboarding, {
            title: name === "c08" ? "Refine Step 2" : "Onboarding screens",
            state: "done",
          }),
        ],
        {
          [SESSIONS.onboarding]: simpleTurn(
            SESSIONS.onboarding,
            name === "c08"
              ? "Make these feel less crowded"
              : "Refine Step 2 using our spacing tokens.",
            name === "c08" ? ["3 layers in Step 2"] : [],
            name === "c08"
              ? "The selected card and two buttons are attached to this request."
              : "I've refined Step 2 and checked the layout. Review agent changes in History.",
            true,
          ),
        },
      ),
      updates: [],
    };
  if (name === "c27") {
    const events = [
      ...simpleTurn(SESSIONS.onboarding, "Design onboarding from @DESIGN.md.", [], "", false),
    ];
    events.push(
      ev(
        SESSIONS.onboarding,
        {
          type: "turn",
          turn: TURN,
          from: "running",
          input: "ask_permission",
          to: "awaiting_permission",
        },
        AGENT_NOW - 50_000,
      ),
    );
    return {
      feed: new FakeAgentFeed([session(SESSIONS.onboarding, { state: "awaiting_permission" })], {
        [SESSIONS.onboarding]: events,
      }),
      updates: [{ type: "question", session: SESSIONS.onboarding, question: PERMISSION_QUESTION }],
    };
  }
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
