// TEST DATA: a scripted feed; the transcripts are fixtures/agent-panel/*.json, made by the Prompt 36–37 engines.
import { copy } from "@vellum/config";
import { describe, expect, it } from "vitest";
import claudeCode from "../../../../fixtures/agent-panel/claude-code.json";
import codex from "../../../../fixtures/agent-panel/codex.json";
import { createAgentController } from "./controller";
import { FakeAgentFeed, type TranscriptFixture } from "./fake";
import type { SessionEvent } from "./log";
import type { SessionInfo } from "./sessions";

const S1 = "01J00000000000000000000S01";
const S2 = "01J00000000000000000000S02";
const T1 = "01J00000000000000000000T01";
const T2 = "01J00000000000000000000T02";
const TEAL = "01J00000000000000000000A01";
const person = { kind: "person", device: "01J00000000000000000000D01" } as const;
const agent = { kind: "agent", agent: TEAL } as const;
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
const designed = (seq: number, turn: string, created: number): SessionEvent => ({
  seq,
  at: seq * 1000,
  origin: { actor: agent, session: S1, turn },
  body: {
    type: "step",
    tool: "insert_nodes",
    arguments: "{}",
    result: "{}",
    startedAt: seq * 1000,
    durationMs: 1000,
    outcome: "done",
    detail: {
      verb: "designed",
      target: { id: "w", name: "Welcome", type: "frame" },
      counts: { created, changed: 0, removed: 0, variables: 0 },
    },
  },
});
const transition = (
  seq: number,
  turn: string,
  from: string,
  input: string,
  to: string,
): SessionEvent => ({
  seq,
  at: seq * 1000,
  origin: { actor: input === "start" ? person : agent, session: S1, turn },
  body: { type: "turn", turn, from, input, to } as SessionEvent["body"],
});
const twoTurns = (): SessionEvent[] => [
  transition(1, T1, "queued", "start", "planning"),
  transition(2, T1, "planning", "no_plan_needed", "running"),
  designed(3, T1, 12),
  transition(4, T1, "running", "done", "done"),
  transition(5, T2, "queued", "start", "planning"),
  transition(6, T2, "planning", "no_plan_needed", "running"),
  designed(7, T2, 5),
  transition(8, T2, "running", "done", "done"),
];

function setup(events = twoTurns()) {
  const feed = new FakeAgentFeed([info(S1), info(S2)], { [S1]: events, [S2]: [] });
  let clock = 100_000;
  const zoomed: string[] = [];
  const reviewed: unknown[] = [];
  const c = createAgentController({
    feed,
    now: () => clock,
    timeZone: "UTC",
    onZoomTo: (node) => zoomed.push(node),
    nameOf: (node) => (node === "s1" ? "Step 1" : null),
    onReviewChanges: (session, turn) => reviewed.push({ session, turn }),
  });
  return { feed, c, zoomed, reviewed, tick: (ms: number) => (clock += ms) };
}

describe("agent controller", () => {
  it("opens the newest session and lists every session grouped", async () => {
    const { c } = setup();
    await c.load();
    expect(c.state.current).toBe(S1);
    expect(c.turns().map((t) => t.id)).toEqual([T1, T2]);
    expect(c.groups().map((g) => g.group)).toEqual(["Recent"]);
  });
  it("undo-turn asks first, then asks the daemon to undo only that turn", async () => {
    const { c, feed } = setup();
    await c.load();
    c.askUndo(T1);
    expect(c.state.confirmUndo).toBe(T1);
    expect(c.undoPrompt()).toEqual({ turn: T1, changes: 12, agent: TEAL });
    await c.confirmUndo();
    expect(feed.requests).toEqual([{ type: "undoTurn", session: S1, turn: T1 }]);
    expect(c.state.confirmUndo).toBeNull();
    // The daemon records the undo in the log; only T1 shows it.
    feed.emit({
      type: "event",
      session: S1,
      event: {
        seq: 9,
        at: 9000,
        origin: { actor: person, session: S1, turn: T1 },
        body: { type: "undo", changed: 12, kept: 0 },
      },
    });
    const [first, second] = c.turns();
    expect(first?.undone).toEqual({ changed: 12, kept: 0 });
    expect(first?.canUndo).toBe(false);
    expect(second?.undone).toBeNull();
    expect(second?.canUndo).toBe(true);
  });
  it("cancelling the confirmation sends nothing", async () => {
    const { c, feed } = setup();
    await c.load();
    c.askUndo(T2);
    c.cancelUndo();
    expect(feed.requests).toEqual([]);
  });
  it("switches sessions, makes new ones and deletes them", async () => {
    const { c, feed } = setup();
    await c.load();
    c.toggleSwitcher();
    expect(c.state.switcher).toBe(true);
    await c.select(S2);
    expect(c.state).toMatchObject({ current: S2, switcher: false });
    expect(c.turns()).toEqual([]);
    await c.newSession();
    expect(c.state.current).toBe(feed.created[0]);
    await c.deleteSession(c.state.current as string);
    expect(feed.requests).toContainEqual({ type: "remove", session: feed.created[0] });
    expect(c.state.current).toBe(S1);
  });
  it("Retry, Retry step, Review changes, zoom to a thumbnail's frame and Stop go to their owners", async () => {
    const { c, feed, zoomed, reviewed } = setup();
    await c.load();
    await c.retry(T2);
    await c.retryStep(T2, 7);
    await c.stop();
    c.reviewChanges(T1);
    c.zoomTo("w");
    expect(feed.requests).toEqual([
      { type: "retry", session: S1, turn: T2 },
      { type: "retryStep", session: S1, turn: T2, seq: 7 },
      { type: "stop", session: S1 },
    ]);
    expect(reviewed).toEqual([{ session: S1, turn: T1 }]);
    expect(zoomed).toEqual(["w"]);
  });
  it("streams text and a working step, then puts the recorded step in its place", async () => {
    const running = twoTurns().slice(0, 6);
    const { c, feed, tick } = setup(running);
    await c.load();
    feed.emit({ type: "live", session: S1, turn: T2, item: { type: "text", text: "Building " } });
    feed.emit({ type: "live", session: S1, turn: T2, item: { type: "text", text: "Step 1." } });
    feed.emit({
      type: "live",
      session: S1,
      turn: T2,
      item: {
        type: "toolStarted",
        call: "c1",
        name: "mcp__vellum__insert_nodes",
        arguments: { parent: "s1" },
      },
    });
    tick(4000);
    let t = c.turns()[1];
    expect(t?.items.map((i) => (i.type === "step" ? i.step.state : i.type))).toEqual([
      "prose",
      "working",
    ]);
    expect(t?.items[1]).toMatchObject({ step: { duration: "4 s", line: "Designing Step 1" } });
    feed.emit({ type: "event", session: S1, event: designed(7, T2, 5) });
    t = c.turns()[1];
    expect(t?.items.map((i) => (i.type === "step" ? i.step.line : i.type))).toEqual([
      "prose",
      "Designed Welcome · 5 layers",
    ]);
  });
  it("ignores events of other sessions and a repeated event", async () => {
    const { c, feed } = setup();
    await c.load();
    const before = c.turns();
    feed.emit({ type: "event", session: S2, event: designed(99, T1, 3) });
    feed.emit({ type: "event", session: S1, event: designed(3, T1, 12) });
    expect(c.turns()).toEqual(before);
  });
  it("says a failed request in words and keeps the state", async () => {
    const { c, feed } = setup();
    await c.load();
    feed.fail = copy.daemonOffline;
    await c.retry(T2);
    expect(c.state.error).toBe(copy.daemonOffline);
  });
});

const fixtures = { "claude-code": claudeCode, codex } as unknown as Record<
  string,
  TranscriptFixture
>;
const fixture = (name: string) => fixtures[name] as TranscriptFixture;

describe("installed-engine transcripts (Prompts 36–37)", () => {
  for (const [name, prose, model] of [
    ["claude-code", "Fixture complete", "sonnet"],
    [
      "codex",
      "I’ll locate the attached canvas state tool and call it once.\n\nFixture complete",
      "fixture-model",
    ],
  ] as const)
    it(`${name}: the stream while it runs, then the stored turn, read the same`, async () => {
      const f = fixture(name);
      const feed = FakeAgentFeed.fromTranscript(f, { upTo: 3 });
      const c = createAgentController({ feed, now: () => f.session.createdAt + 5000 });
      await c.load();
      for (const item of f.live)
        feed.emit({
          type: "live",
          session: f.session.id,
          turn: f.events[0]?.origin.turn as string,
          item,
        });
      let [t] = c.turns();
      expect(t).toMatchObject({ state: "running", prompt: { chips: ["DESIGN.md"] } });
      expect(t?.items).toEqual([
        {
          type: "prose",
          key: "p:0",
          text: f.live.map((l) => ("text" in l ? l.text : "")).join(""),
          streaming: true,
        },
      ]);
      for (const event of f.events.slice(3))
        feed.emit({ type: "event", session: f.session.id, event });
      [t] = c.turns();
      expect(t).toMatchObject({
        state: "done",
        label: "Done",
        model,
        canRetry: true,
        canUndo: false,
      });
      expect(t?.items).toEqual([{ type: "prose", key: "p:0", text: prose, streaming: false }]);
      expect(t?.footerLines[1]).toMatch(/ · \d+k tokens$/);
    });
});
