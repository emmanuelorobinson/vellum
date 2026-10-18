// TEST DATA: a scripted feed standing in for the daemon (Juniper sessions, Teal and Saffron).
import { copy } from "@vellum/config";
import { describe, expect, it } from "vitest";
import { type ComposerScope, createComposerController } from "./composer";
import { createAgentController } from "./controller";
import { capabilityWords, providerStatusLine } from "./engines";
import { FakeAgentFeed } from "./fake";
import type { SessionEvent } from "./log";
import { permissionCard } from "./permissions";
import { selectionLabel } from "./selection";
import type { SessionInfo } from "./sessions";
import { usageView } from "./usage";

const S1 = "01J00000000000000000000S01";
const S2 = "01J00000000000000000000S02";
const T1 = "01J00000000000000000000T01";
const TEAL = "01J00000000000000000000A01";
const SAFFRON = "01J00000000000000000000A03";
const person = { kind: "person", device: "01J00000000000000000000D01" } as const;
const info = (id: string, extra: Partial<SessionInfo> = {}): SessionInfo => ({
  id,
  kind: { type: "engine", engine: "builtin" },
  agent: TEAL,
  title: id === S1 ? "Onboarding screens" : "Token cleanup",
  createdAt: 0,
  updatedAt: id === S1 ? 10 : 5,
  state: "done",
  agents: 1,
  ...extra,
});
const turn = (
  seq: number,
  from: string,
  input: string,
  to: string,
  agent = TEAL,
): SessionEvent => ({
  seq,
  at: seq * 1000,
  origin: {
    actor: input === "start" ? person : { kind: "agent", agent },
    session: S1,
    turn: T1,
  },
  body: { type: "turn", turn: T1, from, input, to } as SessionEvent["body"],
});
const prompt = (seq: number, text: string): SessionEvent => ({
  seq,
  at: seq * 1000,
  origin: { actor: person, session: S1, turn: T1 },
  body: { type: "prompt", text, context: [], images: [] },
});
const running = (): SessionEvent[] => [
  prompt(1, "Design onboarding from @DESIGN.md."),
  turn(2, "queued", "start", "planning"),
  turn(3, "planning", "no_plan_needed", "running"),
];
const finished = (): SessionEvent[] => [...running(), turn(4, "running", "done", "done")];
const QUESTION = {
  id: "q1",
  turn: T1,
  agent: TEAL,
  engine: "builtin",
  action: { readFile: { path: "/Users/sam/code/juniper/DESIGN.md" } },
  kind: "read_repo",
  offered: ["allow_once", "allow_for_session", "deny"],
} as const;

async function setup(
  events: SessionEvent[] = finished(),
  options: { scope?: ComposerScope; sessions?: SessionInfo[] } = {},
) {
  const feed = new FakeAgentFeed(options.sessions ?? [info(S1), info(S2)], {
    [S1]: events,
    [S2]: [],
  });
  feed.skillList = [
    { path: "workflows/design-a-screen.md", name: "Design a screen", group: "builtin" },
    {
      path: "guidelines/starter-minimal.md",
      name: "Starter Minimal guidelines",
      group: "design_system",
    },
    { path: "~/skills/juniper-voice/SKILL.md", name: "Juniper voice", group: "user" },
  ];
  feed.fileList = ["DESIGN.md", "docs/DESIGN-NOTES.md", "src/tokens.css", "src/Card.tsx"];
  let scope: ComposerScope = options.scope ?? {
    selection: null,
    page: { id: "p1", name: "Onboarding" },
  };
  const agent = createAgentController({ feed, now: () => 100_000, timeZone: "UTC" });
  const composer = createComposerController({
    agent,
    feed,
    scope: () => scope,
    documentMentions: (q) =>
      [
        { kind: "frame" as const, label: "Step 2", value: "step2" },
        { kind: "component" as const, label: "Button", value: "c1" },
        { kind: "variable" as const, label: "color/primary", value: "v1" },
      ].filter((m) => m.label.toLowerCase().includes(q.toLowerCase())),
    defaultEngine: {
      engine: "builtin",
      provider: "openrouter",
      model: "deepseek/deepseek-v4.1-flash",
    },
  });
  await agent.load();
  await composer.load();
  return {
    feed,
    agent,
    composer,
    setScope(s: ComposerScope) {
      scope = s;
    },
  };
}
const type = (c: Awaited<ReturnType<typeof setup>>["composer"], text: string) =>
  c.setText(text, text.length);

describe("composer: Send and Stop", () => {
  it("shows Send when nothing works and Stop whenever any agent in the session works", async () => {
    const idle = await setup();
    expect(idle.composer.action()).toEqual({ type: "send" });

    const busy = await setup(running());
    expect(busy.composer.action()).toEqual({ type: "stop", agents: [TEAL] });

    // The session list says it runs before its log has arrived: still Stop.
    const listed = await setup([], { sessions: [info(S1, { state: "running" }), info(S2)] });
    expect(listed.composer.action().type).toBe("stop");

    // Waiting for permission or a plan is still an agent at work.
    const waiting = await setup([
      ...running(),
      turn(4, "running", "ask_permission", "awaiting_permission"),
    ]);
    expect(waiting.composer.action().type).toBe("stop");
  });

  it("offers Stop this agent per agent and Stop all when several agents work", async () => {
    const { feed, composer } = await setup([
      ...running(),
      {
        seq: 4,
        at: 4000,
        origin: { actor: { kind: "agent", agent: TEAL }, session: S1, turn: T1 },
        body: { type: "parallel", event: "lanes", lanes: [{ agent: TEAL }, { agent: SAFFRON }] },
      },
    ]);
    expect(composer.action()).toEqual({ type: "stop", agents: [TEAL, SAFFRON] });
    await composer.stopAgent(SAFFRON);
    await composer.stopAll();
    expect(feed.requests.slice(-2)).toEqual([
      { type: "stopAgent", session: S1, agent: SAFFRON },
      { type: "stop", session: S1 },
    ]);
  });

  it("sends the text, chips, skills, Parallel, Explore and engine, then clears the draft", async () => {
    const { feed, composer } = await setup();
    type(composer, "Make the cards calmer");
    composer.setParallel({ count: 3, mode: "side_by_side" });
    composer.setExplore({ count: 4, vary: "style" });
    composer.setEffort("high");
    await composer.send();
    const sent = feed.sent.at(-1);
    expect(sent).toMatchObject({
      session: S1,
      request: {
        text: "Make the cards calmer",
        context: [{ kind: "page", label: "Onboarding", value: "p1" }],
        skills: [],
        parallel: { count: 3, mode: "side_by_side" },
        explore: { count: 4, vary: "style" },
        engine: {
          engine: "builtin",
          provider: "openrouter",
          model: "deepseek/deepseek-v4.1-flash",
          effort: "high",
        },
        queue: null,
      },
    });
    expect(composer.draft().text).toBe("");
    // Empty requests are not sent.
    await composer.send();
    expect(feed.sent).toHaveLength(1);
  });

  it("asks Queue after this turn or Stop and send now when sending while an agent works", async () => {
    const { feed, composer } = await setup(running());
    type(composer, "Also do Step 3");
    await composer.send();
    expect(feed.sent).toHaveLength(0);
    expect(composer.state.queue).toBe(true);
    // The draft is kept while the choice is open.
    expect(composer.draft().text).toBe("Also do Step 3");
    await composer.resolveQueue("after");
    expect(feed.sent.at(-1)?.request.queue).toBe("after");
    expect(composer.state.queue).toBe(false);

    type(composer, "Actually, stop and do this");
    await composer.send();
    await composer.resolveQueue("now");
    expect(feed.requests.at(-1)).toEqual({ type: "stop", session: S1 });
    expect(feed.sent.at(-1)?.request.queue).toBe("now");

    type(composer, "Never mind");
    await composer.send();
    composer.key("Escape", {});
    expect(composer.state.queue).toBe(false);
    expect(composer.draft().text).toBe("Never mind");
  });

  it("creates a session when none is open", async () => {
    const { feed, composer } = await setup([], { sessions: [] });
    type(composer, "Design a paywall");
    await composer.send();
    expect(feed.created).toHaveLength(1);
    expect(feed.sent.at(-1)?.session).toBe(feed.created[0]);
  });
});

describe("composer: drafts, history and keys", () => {
  it("keeps draft text per session when switching sessions", async () => {
    const { agent, composer } = await setup();
    type(composer, "Draft for onboarding");
    await agent.select(S2);
    expect(composer.draft().text).toBe("");
    type(composer, "Draft for tokens");
    await agent.select(S1);
    expect(composer.draft().text).toBe("Draft for onboarding");
    await agent.select(S2);
    expect(composer.draft().text).toBe("Draft for tokens");
  });

  it("walks prompt history with ↑ and ↓ in an empty composer", async () => {
    const { composer } = await setup();
    type(composer, "Second request");
    await composer.send();
    expect(composer.key("ArrowUp", {})).toBe(true);
    expect(composer.draft().text).toBe("Second request");
    expect(composer.key("ArrowUp", {})).toBe(true);
    expect(composer.draft().text).toBe("Design onboarding from @DESIGN.md.");
    expect(composer.key("ArrowUp", {})).toBe(true);
    expect(composer.draft().text).toBe("Design onboarding from @DESIGN.md.");
    expect(composer.key("ArrowDown", {})).toBe(true);
    expect(composer.draft().text).toBe("Second request");
    expect(composer.key("ArrowDown", {})).toBe(true);
    expect(composer.draft().text).toBe("");
    // With text in the field, arrows move the caret instead.
    type(composer, "typing");
    expect(composer.key("ArrowUp", {})).toBe(false);
  });

  it("sends on Enter and leaves Shift+Enter to add a line", async () => {
    const { feed, composer } = await setup();
    type(composer, "One line");
    expect(composer.key("Enter", { shift: true })).toBe(false);
    expect(composer.key("Enter", {})).toBe(true);
    await composer.idle();
    expect(feed.sent.at(-1)?.request.text).toBe("One line");
  });
});

describe("composer: context chips, @ mentions and / skills", () => {
  it("adds the selection automatically, changes the placeholder, and lets the person remove it", async () => {
    const { composer, setScope } = await setup();
    expect(composer.placeholder()).toBe(copy.composerPlaceholder);
    expect(composer.chips().map((c) => c.label)).toEqual(["Onboarding"]);
    setScope({
      selection: { ids: ["card", "a", "b"], label: "3 layers in Step 2" },
      page: { id: "p1", name: "Onboarding" },
    });
    expect(composer.placeholder()).toBe(copy.selectionPlaceholder);
    const chips = composer.chips();
    expect(chips.map((c) => c.label)).toEqual(["3 layers in Step 2"]);
    composer.removeChip(chips[0]?.key as string);
    expect(composer.chips()).toEqual([]);
    // A different selection is a new chip.
    setScope({ selection: { ids: ["title"], label: "Title" }, page: null });
    expect(composer.chips().map((c) => c.label)).toEqual(["Title"]);
  });

  it("@ lists files, frames, components and variables, and a pick becomes text and a chip", async () => {
    const { composer } = await setup();
    type(composer, "Design onboarding from @DES");
    await composer.idle();
    const menu = composer.state.menu;
    expect(menu?.type).toBe("mention");
    expect(composer.menuItems().map((i) => i.line)).toEqual([
      "DESIGN.md · File",
      "docs/DESIGN-NOTES.md · File",
    ]);
    composer.key("ArrowDown", {});
    composer.key("ArrowUp", {});
    expect(composer.key("Enter", {})).toBe(true);
    expect(composer.draft().text).toBe("Design onboarding from @DESIGN.md ");
    expect(composer.chips().map((c) => [c.kind, c.label])).toContainEqual(["file", "DESIGN.md"]);
    expect(composer.state.menu).toBeNull();

    type(composer, "Use @col");
    await composer.idle();
    expect(composer.menuItems().map((i) => i.line)).toEqual(["color/primary · Variable"]);
  });

  it("/ at the start opens the skills menu grouped Built-in, Design system, Yours, and a pick is a chip", async () => {
    const { feed, composer } = await setup();
    type(composer, "/");
    expect(composer.state.menu?.type).toBe("skills");
    expect(composer.skillGroups().map((g) => [g.group, g.skills.map((s) => s.name)])).toEqual([
      ["Built-in", ["Design a screen"]],
      ["Design system", ["Starter Minimal guidelines"]],
      ["Yours", ["Juniper voice"]],
    ]);
    type(composer, "/juni");
    expect(composer.menuItems().map((i) => i.line)).toEqual(["Juniper voice"]);
    composer.key("Enter", {});
    expect(composer.draft().text).toBe("");
    expect(composer.chips().map((c) => c.label)).toContain("Skill · Juniper voice");
    type(composer, "Design onboarding from @DESIGN.md.");
    await composer.send();
    expect(feed.sent.at(-1)?.request.skills).toEqual(["~/skills/juniper-voice/SKILL.md"]);

    // "/" later in the text is just a slash.
    type(composer, "a/b");
    expect(composer.state.menu).toBeNull();
  });

  it("Add skill… asks the host for a folder and lists it under Yours", async () => {
    const { feed, composer } = await setup();
    feed.nextSkill = { path: "~/skills/a11y/SKILL.md", name: "Accessibility", group: "user" };
    await composer.addSkill();
    expect(
      composer
        .skillGroups()
        .at(-1)
        ?.skills.map((s) => s.name),
    ).toEqual(["Juniper voice", "Accessibility"]);
  });

  it("attaching files from the host or a drop adds chips", async () => {
    const { composer } = await setup();
    composer.attach([{ kind: "attachment", label: "hero.png", value: "attachment:1" }]);
    expect(composer.chips().map((c) => c.label)).toEqual(["Onboarding", "hero.png"]);
  });
});

describe("composer: Permission card", () => {
  it("blocks the turn until the person decides, and the card stays until the daemon has the answer", async () => {
    const { feed, agent, composer } = await setup([
      ...running(),
      turn(4, "running", "ask_permission", "awaiting_permission"),
    ]);
    feed.emit({ type: "question", session: S1, question: QUESTION });
    expect(composer.questionFor(T1)?.id).toBe("q1");
    expect(agent.turns().at(-1)?.state).toBe("awaiting_permission");
    // Sending meanwhile asks first: the turn is not done.
    type(composer, "Go on");
    await composer.send();
    expect(composer.state.queue).toBe(true);
    composer.key("Escape", {});

    const answering = composer.answer("q1", "allow_once");
    expect(composer.answering("q1")).toBe(true);
    await answering;
    expect(feed.requests.at(-1)).toEqual({
      type: "answer",
      session: S1,
      question: "q1",
      answer: "allow_once",
    });
    // Still blocked: only the daemon's record of the decision lets the turn go on.
    expect(composer.questionFor(T1)?.id).toBe("q1");
    expect(agent.turns().at(-1)?.state).toBe("awaiting_permission");
    // A second press does not answer twice.
    await composer.answer("q1", "deny");
    expect(feed.requests.filter((r) => r.type === "answer")).toHaveLength(1);

    feed.emit({ type: "answered", session: S1, question: "q1" });
    feed.emit({
      type: "event",
      session: S1,
      event: {
        seq: 5,
        at: 5000,
        origin: { actor: person, session: S1, turn: T1 },
        body: {
          type: "permission",
          action: QUESTION.action,
          kind: "read_repo",
          answer: "allow_once",
          source: { kind: "person" },
          engine: "builtin",
        },
      },
    });
    feed.emit({
      type: "event",
      session: S1,
      event: turn(6, "awaiting_permission", "permission_answered", "running"),
    });
    expect(composer.questionFor(T1)).toBeNull();
    expect(agent.turns().at(-1)?.state).toBe("running");
    const decided = agent
      .turns()
      .at(-1)
      ?.items.find((i) => i.type === "permission");
    expect(decided).toMatchObject({
      type: "permission",
      card: { caption: "Allowed", message: "Permission allowed once" },
    });
  });

  it("writes the card in plain words with the exact path or command", () => {
    expect(
      permissionCard({ readFile: { path: "/Users/sam/code/juniper/DESIGN.md" } }, "Teal"),
    ).toEqual({
      sentence: "Teal wants to read DESIGN.md",
      target: "/Users/sam/code/juniper/DESIGN.md",
    });
    expect(permissionCard({ writeFile: { path: "/r/src/tokens.css" } }, "Teal").sentence).toBe(
      "Teal wants to write tokens.css",
    );
    expect(
      permissionCard(
        { runCommand: { program: "pnpm", args: ["build"], cwd: "/r" } },
        "Claude Code",
      ),
    ).toEqual({ sentence: "Claude Code wants to run pnpm build", target: "pnpm build" });
    expect(
      permissionCard({ network: { url: "https://api.example.test/x" } }, "Teal").sentence,
    ).toBe("Teal wants to reach api.example.test");
  });

  it("shows the skip-all warning while skip-all is on and turns prompts back on", async () => {
    const { feed, composer } = await setup();
    expect(composer.state.skipAll).toBe(false);
    feed.emit({ type: "permissionSettings", skipAll: true });
    expect(composer.state.skipAll).toBe(true);
    await composer.turnPromptsOn();
    expect(feed.requests.at(-1)).toEqual({ type: "setSkipAll", on: false });
    expect(composer.state.skipAll).toBe(false);
  });
});

describe("composer: prefill and skip-all from Settings", () => {
  it("prefill puts words in the field for the person to edit, and asks the view to focus it", async () => {
    const { composer } = await setup();
    const focus = composer.state.focus;
    composer.prefill("Make the cards rounder");
    expect(composer.draft().text).toBe("Make the cards rounder");
    expect(composer.state.focus).toBe(focus + 1);
    expect(composer.state.sending).toBe(false);
  });
  it("turning skip-all on goes through the feed, and a failed request leaves it off", async () => {
    const { feed, composer } = await setup();
    await composer.setSkipAll(true);
    expect(feed.requests.at(-1)).toEqual({ type: "setSkipAll", on: true });
    expect(composer.state.skipAll).toBe(true);
    await composer.setSkipAll(false);
    expect(composer.state.skipAll).toBe(false);
  });
});

describe("composer: engine, model and providers", () => {
  it("names capabilities as words", () => {
    expect(
      capabilityWords({ vision: true, fast: true, reasoning: false, contextTokens: 1_000_000 }),
    ).toBe("Vision · Fast · Large context");
    expect(
      capabilityWords({ vision: false, fast: true, reasoning: true, contextTokens: 64_000 }),
    ).toBe("Fast · Reasoning");
    expect(
      capabilityWords({
        vision: false,
        fast: false,
        reasoning: false,
        contextTokens: null,
        local: true,
      }),
    ).toBe("On your computer");
  });

  it("says each provider's status in words", () => {
    expect(providerStatusLine("connected")).toBe("Connected");
    expect(providerStatusLine("api_key_set")).toBe("API key set · not tested");
    expect(providerStatusLine("signed_in")).toBe("Signed in");
    expect(providerStatusLine("configured_externally")).toBe(
      "Configured outside Vellum (not tested)",
    );
    expect(providerStatusLine("failed")).toBe("Failed");
  });

  it("chooses a model and effort, and warns when the provider changes mid-session", async () => {
    const { composer } = await setup();
    expect(composer.modelLine()).toBe("OpenRouter · deepseek/deepseek-v4.1-flash");
    expect(composer.switchWarning()).toBe(false);
    composer.choose({ engine: "builtin", provider: "anthropic", model: "claude-sonnet" });
    expect(composer.modelLine()).toBe("Anthropic · Claude Sonnet");
    // The session already has turns, made by another provider.
    expect(composer.switchWarning()).toBe(true);
    composer.choose({ engine: "claude-code", model: "default" });
    expect(composer.modelLine()).toBe("Claude Code");
  });

  it("never keeps an API key after it is entered", async () => {
    const { feed, composer } = await setup();
    const key = "sk-or-v1-TESTDATA-not-a-real-key-0123456789";
    const status = await composer.saveProvider({ provider: "openrouter", key });
    expect(status).toBe("connected");
    expect(feed.keys.openrouter).toBe(key);
    expect(JSON.stringify(composer.state)).not.toContain(key);
    expect(JSON.stringify(feed.requests)).not.toContain(key);
    expect(composer.providerStatus("openrouter")).toBe("connected");
    await composer.clearKey("openrouter");
    expect(composer.providerStatus("openrouter")).toBe("configured_externally");
  });
});

describe("composer: usage meter", () => {
  it("fills the ring by context and labels the cost as an estimate", () => {
    const view = usageView({
      contextTokens: 8100,
      window: 45_000,
      conversation: 4200,
      files: 2800,
      screenshots: 1100,
      estimate: 40_000,
    });
    expect(view).toEqual({
      fraction: 0.18,
      title: "Context usage · 18%",
      rows: [
        ["Conversation", "4.2k tokens"],
        ["Files", "2.8k tokens"],
        ["Screenshots", "1.1k tokens"],
      ],
      cost: `${copy.costLabel} · $0.04`,
      note: "Estimate for this session. Your provider bills actual usage.",
      plan: null,
    });
    expect(
      usageView({
        contextTokens: 0,
        window: null,
        conversation: 0,
        files: 0,
        screenshots: 0,
        estimate: 3000,
      }).cost,
    ).toBe(`${copy.costLabel} · less than $0.01`);
  });

  it("shows plan usage instead of cost for an installed engine on a subscription", () => {
    const view = usageView(
      {
        contextTokens: 50_000,
        window: 200_000,
        conversation: 50_000,
        files: 0,
        screenshots: 0,
        estimate: null,
        plan: { used: 0.4, resetsAt: Date.UTC(2026, 9, 18, 18, 0) },
      },
      { timeZone: "UTC" },
    );
    expect(view.cost).toBeNull();
    expect(view.plan).toBe("40% of your plan's limit · resets at 18:00");
  });
});

describe("selection label", () => {
  const doc = {
    pages: [
      {
        id: "p1",
        name: "Onboarding",
        children: [
          {
            type: "frame",
            id: "step2",
            name: "Step 2",
            children: [
              { type: "rect", id: "card", name: "Habit card" },
              {
                type: "frame",
                id: "row",
                name: "Row",
                children: [{ type: "text", id: "t", name: "Title" }],
              },
            ],
          },
          { type: "frame", id: "step3", name: "Step 3" },
        ],
      },
    ],
  };
  it("names one layer, or counts layers in their frame", () => {
    expect(selectionLabel(doc, "p1", ["card"])).toBe("Habit card");
    expect(selectionLabel(doc, "p1", ["card", "row", "t"])).toBe("3 layers in Step 2");
    expect(selectionLabel(doc, "p1", ["step2", "step3"])).toBe("2 frames");
    expect(selectionLabel(doc, "p1", ["card", "step3"])).toBe("2 layers");
    expect(selectionLabel(doc, "p1", [])).toBeNull();
  });
});

describe("inline prompt", () => {
  it("runs in the current session, then shows progress and Undo", async () => {
    const { feed, composer } = await setup(finished(), {
      scope: { selection: { ids: ["card", "a", "b"], label: "3 layers in Step 2" }, page: null },
    });
    composer.openInline();
    expect(composer.state.inline).toEqual({ phase: "edit", text: "" });
    composer.setInlineText("Make these feel less crowded");
    await composer.sendInline();
    const sent = feed.sent.at(-1);
    expect(sent?.session).toBe(S1);
    expect(sent?.request.context).toEqual([
      { kind: "selection", label: "3 layers in Step 2", value: "card,a,b" },
    ]);
    expect(composer.state.inline?.phase).toBe("sent");
    expect(composer.inlineLine()).toBe("Refining selected layers…");
    await composer.undoInline();
    expect(feed.requests.at(-1)).toMatchObject({ type: "undoTurn", session: S1 });
    expect(composer.state.inline).toBeNull();
  });

  it("does not open without a selection", async () => {
    const { composer } = await setup();
    composer.openInline();
    expect(composer.state.inline).toBeNull();
  });
});
