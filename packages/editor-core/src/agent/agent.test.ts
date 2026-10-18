// TEST DATA throughout: sessions, turns and steps are written by hand in the session log's JSON (the Rust codec's).
import { describe, expect, it } from "vitest";
import { type EventBody, parseEvent, type SessionEvent } from "./log";
import { createScrollFollow } from "./scroll";
import { groupSessions, type SessionInfo, sessionLine } from "./sessions";
import { describeStep, formatDuration, formatTokens } from "./steps";
import { buildTurns, type TurnItem } from "./turns";

const SESSION = "01J00000000000000000000S01";
const T1 = "01J00000000000000000000T01";
const T2 = "01J00000000000000000000T02";
const TEAL = "01J00000000000000000000A01";
const person = { kind: "person", device: "01J00000000000000000000D01" } as const;
const agent = { kind: "agent", agent: TEAL } as const;

let seq = 0;
export function ev(
  body: EventBody,
  at: number,
  turn: string | null = T1,
  actor: SessionEvent["origin"]["actor"] = agent,
): SessionEvent {
  return { seq: ++seq, at, origin: { actor, session: SESSION, turn }, body };
}
const step = (
  tool: string,
  detail: Record<string, unknown>,
  extra: Partial<Extract<EventBody, { type: "step" }>> = {},
): EventBody => ({
  type: "step",
  tool,
  arguments: "{}",
  result: "{}",
  startedAt: 0,
  durationMs: 1000,
  outcome: "done",
  detail: {
    tool,
    outcome: "done",
    counts: { created: 0, changed: 0, removed: 0, variables: 0 },
    startedAt: 0,
    durationMs: 1000,
    ...detail,
  },
  ...extra,
});
const turn = (from: string, input: string, to: string, id = T1): EventBody =>
  ({ type: "turn", turn: id, from, input, to }) as EventBody;
const welcome = { id: "welcome", name: "Welcome", type: "frame" };

describe("session log events", () => {
  it("reads the Rust codec's JSON and refuses kinds this version does not know", () => {
    const e = parseEvent({
      seq: 1,
      at: 5,
      origin: { actor: person, session: SESSION, turn: T1 },
      body: { type: "prompt", text: "Hi", context: ["DESIGN.md"], images: [] },
    });
    expect(e.body).toEqual({ type: "prompt", text: "Hi", context: ["DESIGN.md"], images: [] });
    expect(() =>
      parseEvent({ seq: 1, at: 5, origin: { actor: person }, body: { type: "gossip" } }),
    ).toThrow(/gossip/);
    expect(() => parseEvent({ seq: "1", at: 5, origin: {}, body: { type: "undo" } })).toThrow();
  });
});

describe("step rows", () => {
  it("writes the design's lines for each verb in the panel's own copy", () => {
    const line = (tool: string, d: Record<string, unknown>, a = "{}") =>
      describeStep(step(tool, d, { arguments: a }) as never).line;
    expect(line("read_file", { verb: "read" }, '{"path":"DESIGN.md"}')).toBe("Read DESIGN.md");
    expect(line("read_skill", { verb: "read" }, '{"path":"guidelines"}')).toBe(
      "Read design-system guidelines",
    );
    expect(line("claim_frame", { verb: "claimed", target: welcome })).toBe("Claimed frame Welcome");
    expect(
      line("insert_nodes", {
        verb: "designed",
        target: welcome,
        counts: { created: 28, changed: 0, removed: 0, variables: 0 },
      }),
    ).toBe("Designed Welcome · 28 layers");
    expect(
      line("set_variables", {
        verb: "updated_variables",
        counts: { created: 0, changed: 0, removed: 0, variables: 4 },
      }),
    ).toBe("Updated 4 tokens");
    expect(line("screenshot", { verb: "checked_screenshot" })).toBe("Checked screenshot");
  });
  it("gives each step its icon, state and duration", () => {
    const d = describeStep(
      step(
        "insert_nodes",
        { verb: "designed", target: welcome, thumbnail: { node: "welcome", scale: 0.25 } },
        {
          durationMs: 18_400,
          thumbnail: "a".repeat(64),
        },
      ) as never,
    );
    expect(d).toMatchObject({
      icon: "layout-template",
      state: "done",
      duration: "18 s",
      kind: "designed",
      thumbnail: { node: "welcome", image: "a".repeat(64), name: "Welcome" },
    });
  });
  it("shows a checked screenshot's verdict, a layout check's problems and a script's change count", () => {
    expect(
      describeStep(
        step(
          "screenshot",
          { verb: "checked_screenshot", message: "Fixed: CTA text overflowed at 375 px" },
          {
            thumbnail: "b".repeat(64),
          },
        ) as never,
      ),
    ).toMatchObject({
      kind: "screenshot",
      verdict: "Fixed: CTA text overflowed at 375 px",
      icon: "scan-eye",
    });
    expect(
      describeStep(
        step("snapshot_layout", {
          verb: "checked_layout",
          message: "2 problems · text overflow in Step 2 title fixed",
        }) as never,
      ),
    ).toMatchObject({
      kind: "layout",
      line: "Checked layout",
      problems: "2 problems · text overflow in Step 2 title fixed",
    });
    expect(
      describeStep(
        step("execute", {
          verb: "ran_script",
          script: "insert(…)",
          counts: { created: 3, changed: 2, removed: 0, variables: 0 },
        }) as never,
      ),
    ).toMatchObject({ kind: "script", line: "Ran script · 5 changes", script: "insert(…)" });
  });
  it("says a failed step plainly and marks a refused one as a warning", () => {
    const failed = describeStep(
      step(
        "fill_image",
        { verb: "added_asset", outcome: "failed", error: "The image service did not answer" },
        {
          outcome: "failed",
        },
      ) as never,
    );
    expect(failed).toMatchObject({
      state: "failed",
      icon: "image-plus",
      line: "Image request failed",
      error: "The image service did not answer",
    });
    const refused = describeStep(
      step(
        "update_nodes",
        { verb: "updated", outcome: "failed", error: "Frame leased by Saffron" },
        {
          outcome: "rejected",
        },
      ) as never,
    );
    expect(refused.state).toBe("warning");
  });
  it("formats durations and token counts the way the footer does", () => {
    expect(formatDuration(400)).toBe("1 s");
    expect(formatDuration(18_000)).toBe("18 s");
    expect(formatDuration(72_000)).toBe("1 m 12 s");
    expect(formatTokens(38_210)).toBe("38k");
    expect(formatTokens(812)).toBe("812");
    expect(formatTokens(1_250_000)).toBe("1.3M");
  });
});

describe("turns", () => {
  const finished = (): SessionEvent[] => [
    ev(
      {
        type: "prompt",
        text: "Design onboarding",
        context: ["DESIGN.md", "Starter Minimal"],
        images: [],
      },
      0,
      T1,
      person,
    ),
    ev(turn("queued", "start", "planning"), 1000, T1, person),
    ev(turn("planning", "no_plan_needed", "running"), 2000),
    ev(step("read_file", { verb: "read" }, { arguments: '{"path":"DESIGN.md"}' }), 3000),
    ev(
      step("insert_nodes", {
        verb: "designed",
        target: welcome,
        counts: { created: 28, changed: 0, removed: 0, variables: 0 },
      }),
      20_000,
    ),
    ev(
      step("update_nodes", {
        verb: "updated",
        target: { id: "step1", name: "Step 1", type: "frame" },
        counts: { created: 0, changed: 19, removed: 0, variables: 0 },
      }),
      40_000,
    ),
    ev(
      {
        type: "usage",
        model: "deepseek/deepseek-v4.1-flash",
        tokens: { input: 30_000, output: 2_000, cacheRead: 6_000, cacheWrite: 0 },
      },
      50_000,
    ),
    ev(
      {
        type: "messages",
        messages: [
          { role: "user", parts: [{ text: "Design onboarding" }] },
          {
            role: "assistant",
            parts: [
              { text: "I'll read the spec first." },
              {
                toolCall: {
                  id: "c1",
                  name: "mcp__vellum__read_file",
                  arguments: {},
                  metadata: null,
                },
              },
              { toolCall: { id: "c2", name: "insert_nodes", arguments: {}, metadata: null } },
              { text: "Welcome is built. Now Step 1." },
              { toolCall: { id: "c3", name: "update_nodes", arguments: {}, metadata: null } },
            ],
          },
        ],
      },
      71_000,
    ),
    ev(turn("running", "done", "done"), 72_000),
  ];

  it("interleaves prose and steps in the order they happened (decision A)", () => {
    const [t] = buildTurns(finished(), {});
    const kinds = t?.items.map((i: TurnItem) =>
      i.type === "prose" ? `p:${i.text}` : `s:${i.type === "step" ? i.step.line : ""}`,
    );
    expect(kinds).toEqual([
      "p:I'll read the spec first.",
      "s:Read DESIGN.md",
      "s:Designed Welcome · 28 layers",
      "p:Welcome is built. Now Step 1.",
      "s:Updated Step 1 · 19 layers",
    ]);
  });
  it("carries the prompt with its chips, the model, the state and the footer", () => {
    const [t] = buildTurns(finished(), {});
    expect(t).toMatchObject({
      id: T1,
      prompt: { text: "Design onboarding", chips: ["DESIGN.md", "Starter Minimal"] },
      agents: [TEAL],
      model: "deepseek/deepseek-v4.1-flash",
      state: "done",
      label: "Done",
      latest: true,
      footer: { changes: 47, frames: 2, durationMs: 72_000, tokens: 38_000 },
      canUndo: true,
      canRetry: true,
    });
    expect(t?.footerLines).toEqual(["Made 47 changes · 2 frames", "1 m 12 s · 38k tokens"]);
  });
  it("offers Retry only on the latest turn and marks an undone turn", () => {
    const events = [
      ...finished(),
      ev({ type: "prompt", text: "Again", context: [], images: [] }, 80_000, T2, person),
      ev(turn("queued", "start", "planning", T2), 80_100, T2, person),
      ev(turn("planning", "no_plan_needed", "running", T2), 80_200, T2),
      ev(turn("running", "done", "done", T2), 81_000, T2),
      ev({ type: "undo", changed: 47, kept: 0 }, 90_000, T1, person),
    ];
    const [first, second] = buildTurns(events, {});
    expect(first).toMatchObject({
      canRetry: false,
      undone: { changed: 47, kept: 0 },
      canUndo: false,
    });
    expect(second).toMatchObject({ latest: true, canRetry: true, canUndo: false });
  });
  it("reads a stopped turn as Stopped by you and keeps Undo this turn", () => {
    const [t] = buildTurns(
      [
        ev({ type: "prompt", text: "x", context: [], images: [] }, 0, T1, person),
        ev(turn("queued", "start", "planning"), 1, T1, person),
        ev(turn("planning", "no_plan_needed", "running"), 2),
        ev(
          step("insert_nodes", {
            verb: "designed",
            target: welcome,
            counts: { created: 3, changed: 0, removed: 0, variables: 0 },
          }),
          3,
        ),
        ev(turn("running", "stop", "stopped"), 4, T1, person),
      ],
      {},
    );
    expect(t).toMatchObject({ state: "stopped", label: "Stopped by you", canUndo: true });
  });
  it("shows the live stream of a running turn: text, then a working step, then its record", () => {
    const events = [
      ev({ type: "prompt", text: "x", context: [], images: [] }, 0, T1, person),
      ev(turn("queued", "start", "planning"), 1, T1, person),
      ev(turn("planning", "no_plan_needed", "running"), 2),
    ];
    const live = {
      [T1]: [
        { type: "text" as const, text: "I've built " },
        { type: "text" as const, text: "Welcome." },
        { type: "working" as const, call: "c1", tool: "insert_nodes", startedAt: 10_000 },
      ],
    };
    const [t] = buildTurns(events, { live, now: 16_000 });
    expect(t?.state).toBe("running");
    expect(t?.label).toBe("Working");
    expect(t?.items.map((i) => i.type)).toEqual(["prose", "step"]);
    expect(t?.items[0]).toMatchObject({ text: "I've built Welcome.", streaming: true });
    expect(t?.items[1]).toMatchObject({
      step: { state: "working", line: "Designing", duration: "6 s" },
    });
    expect(t?.footer).toBeNull();
    const stuck = buildTurns(events, { live, now: 10_000 + 121_000 })[0]?.items[1];
    expect(stuck).toMatchObject({ step: { stuck: true } });
  });
  it("groups long runs of the same kind of step", () => {
    const events = [
      ev({ type: "prompt", text: "x", context: [], images: [] }, 0, T1, person),
      ...["a.md", "b.md", "c.md"].map((p, i) =>
        ev(step("read_file", { verb: "read" }, { arguments: JSON.stringify({ path: p }) }), i + 1),
      ),
      ...Array.from({ length: 6 }, (_, i) =>
        ev(
          step("update_nodes", {
            verb: "updated",
            target: welcome,
            counts: { created: 0, changed: 1, removed: 0, variables: 0 },
          }),
          10 + i,
        ),
      ),
    ];
    const items = buildTurns(events, {})[0]?.items ?? [];
    expect(items[0]).toMatchObject({ type: "group", line: "Read 3 files", steps: { length: 3 } });
    expect(items[1]).toMatchObject({ type: "group", line: "Updated Welcome · 6 steps" });
  });
  it("shows an external agent's steps as one turn with no prompt and no Retry", () => {
    const claude = { kind: "agent", agent: "01J00000000000000000000A09" } as const;
    const [t] = buildTurns(
      [
        ev(
          step("read_file", { verb: "read" }, { arguments: '{"path":"DESIGN.md"}' }),
          1,
          null,
          claude,
        ),
        ev(
          step("insert_nodes", {
            verb: "designed",
            target: { id: "s2", name: "Step 2", type: "frame" },
            counts: { created: 28, changed: 0, removed: 0, variables: 0 },
          }),
          18_000,
          null,
          claude,
        ),
      ],
      { external: true },
    );
    expect(t).toMatchObject({ external: true, prompt: null, canRetry: false, canUndo: true });
    expect(t?.footerLines).toEqual(["Made 28 changes · 1 frame · 18 s"]);
  });
});

describe("sessions", () => {
  const s = (id: string, extra: Partial<SessionInfo>): SessionInfo => ({
    id,
    kind: { type: "engine", engine: "builtin" },
    agent: TEAL,
    title: id,
    createdAt: 0,
    updatedAt: 0,
    state: null,
    agents: 1,
    ...extra,
  });
  const now = Date.UTC(2026, 9, 18, 10, 20);
  it("groups Running, Needs you and Recent, newest first", () => {
    const groups = groupSessions([
      s("Token cleanup", { state: "done", updatedAt: now - 60_000 * 8 }),
      s("Onboarding screens", { state: "running", agents: 4, updatedAt: now - 1000 }),
      s("Claude Code", {
        kind: { type: "external", client: "Claude Code" },
        connected: true,
        updatedAt: now - 2000,
      }),
      s("Paywall variants", { state: "running", waiting: "choice", updatedAt: now - 5000 }),
      s("Old", { state: "failed", updatedAt: now - 86_400_000 * 3 }),
    ]);
    expect(groups.map((g) => [g.group, g.sessions.map((x) => x.id)])).toEqual([
      ["Running", ["Onboarding screens", "Claude Code"]],
      ["Needs you", ["Paywall variants"]],
      ["Recent", ["Token cleanup", "Old"]],
    ]);
  });
  it("writes each row's second line", () => {
    const opts = { now, timeZone: "UTC" };
    expect(sessionLine(s("a", { state: "running", agents: 4 }), opts)).toBe("4 agents · working");
    expect(sessionLine(s("a", { state: "running" }), opts)).toBe("working");
    expect(
      sessionLine(
        s("a", {
          kind: { type: "external", client: "Claude Code" },
          connected: true,
          createdAt: now - 8 * 60_000,
        }),
        opts,
      ),
    ).toBe("external · 8 minutes");
    expect(sessionLine(s("a", { state: "running", waiting: "choice" }), opts)).toBe(
      "Choose a variant",
    );
    expect(sessionLine(s("a", { state: "awaiting_permission" }), opts)).toBe(
      "Waiting for permission",
    );
    expect(
      sessionLine(s("a", { state: "done", updatedAt: Date.UTC(2026, 9, 18, 10, 12) }), opts),
    ).toBe("Done · today, 10:12");
    expect(
      sessionLine(s("a", { state: "stopped", updatedAt: Date.UTC(2026, 9, 17, 9, 5) }), opts),
    ).toBe("Stopped by you · yesterday, 09:05");
    expect(
      sessionLine(s("a", { state: "failed", updatedAt: Date.UTC(2026, 9, 2, 9, 5) }), opts),
    ).toBe("Failed · Oct 2, 09:05");
  });
});

describe("scroll follow", () => {
  it("does not mistake its own scroll, reported after more content arrived, for the reader's", () => {
    const f = createScrollFollow();
    expect(f.content({ height: 1000, client: 400 })).toEqual({ scrollTo: 600 });
    // The content grew again before the scroll event of the move to 600 came in.
    f.scrolled({ top: 600, height: 1200, client: 400 });
    expect(f.state.following).toBe(true);
    expect(f.content({ height: 1200, client: 400 })).toEqual({ scrollTo: 800 });
  });
  it("follows new content at the bottom and never moves the reader who scrolled up", () => {
    const f = createScrollFollow();
    expect(f.content({ height: 1000, client: 400 })).toEqual({ scrollTo: 600 });
    f.scrolled({ top: 600, height: 1000, client: 400 });
    expect(f.content({ height: 1100, client: 400 })).toEqual({ scrollTo: 700 });
    expect(f.state.latest).toBe(false);
    f.scrolled({ top: 200, height: 1100, client: 400 });
    expect(f.content({ height: 1300, client: 400 })).toEqual({ scrollTo: null });
    expect(f.state).toEqual({ following: false, latest: true });
    expect(f.jump({ height: 1300, client: 400 })).toEqual({ scrollTo: 900 });
    expect(f.state).toEqual({ following: true, latest: false });
    f.scrolled({ top: 880, height: 1300, client: 400 });
    expect(f.state.following).toBe(true);
  });
});
