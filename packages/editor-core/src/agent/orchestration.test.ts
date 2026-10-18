// TEST DATA: the session codec's plan/parallel records, without a daemon or a model.
import { describe, expect, it } from "vitest";
import { createAgentController } from "./controller";
import { FakeAgentFeed } from "./fake";
import type { SessionEvent } from "./log";
import { editPlan, reduceOrchestration, validatePlan } from "./orchestration";
import { buildTurns } from "./turns";

const plan = {
  title: "Screens",
  items: [
    { id: 1, name: "Start", page: "page", intent: "Introduce", width: 200, height: 300 },
    { id: 2, name: "Finish", page: "page", intent: "Complete", width: 200, height: 300 },
  ],
};
const event = (seq: number, body: unknown, agent = "agent1"): SessionEvent =>
  ({
    seq,
    at: seq,
    origin: { actor: { kind: "agent", agent }, session: "session", turn: "turn" },
    body,
  }) as SessionEvent;
const proposed = event(1, { type: "plan", event: "proposed", plan });
const lanes = event(3, {
  type: "parallel",
  event: "lanes",
  mode: "split",
  lanes: [
    { agent: "agent1", name: "First", model: "model1", items: [1] },
    { agent: "agent2", name: "Second", model: "model2", items: [2] },
  ],
});
describe("plans and parallel turns", () => {
  it("keeps proposals out of the document and clears placeholders on terminal events", () => {
    expect(reduceOrchestration([proposed]).placeholders).toEqual([]);
    const built = event(2, {
      type: "plan",
      event: "built",
      plan,
      placeholders: [
        {
          item: 1,
          frame: "start",
          name: "Start",
          page: "page",
          bounds: { x: 0, y: 0, width: 200, height: 300 },
          agent: "agent1",
          turn: "turn",
        },
        {
          item: 2,
          frame: "finish",
          name: "Finish",
          page: "page",
          bounds: { x: 220, y: 0, width: 200, height: 300 },
          agent: "agent2",
          turn: "turn",
        },
      ],
    });
    const events = [proposed, built, event(4, { type: "plan", event: "ticked", items: [1, 1] })];
    expect(reduceOrchestration(events).done).toEqual([1]);
    expect(reduceOrchestration(events).placeholders.map((p) => p.item)).toEqual([2]);
    expect(
      reduceOrchestration([...events, event(5, { type: "plan", event: "abandoned" })]).placeholders,
    ).toEqual([]);
  });
  it("edits, reorders and removes items immutably; invalid sizes and empty plans cannot Build", () => {
    const edited = editPlan(plan, {
      type: "update",
      id: 1,
      patch: { name: "New", intent: "Updated" },
    });
    expect(plan.items[0]?.name).toBe("Start");
    expect(editPlan(edited, { type: "move", id: 2, index: 0 }).items[0]?.id).toBe(2);
    expect(editPlan(edited, { type: "remove", id: 1 }).items).toHaveLength(1);
    expect(validatePlan({ ...plan, items: [] })).toBeTruthy();
    expect(validatePlan({ ...plan, items: [{ ...plan.items[0]!, width: NaN }] })).toBeTruthy();
  });
  it("folds lanes, progress and alternatives in origin order, with separate steps per agent", () => {
    const state = reduceOrchestration([
      lanes,
      proposed,
      event(4, { type: "plan", event: "ticked", items: [1, 1, 999] }),
      event(5, { type: "parallel", event: "laneEnded", agent: "agent2", end: "failed" }),
      event(6, {
        type: "parallel",
        event: "option",
        option: 2,
        agent: "agent2",
        frame: "b",
        label: "Warmer",
      }),
    ]);
    expect(state.done).toEqual([1]);
    expect(state.lanes[1]?.end).toBe("failed");
    expect(state.options[0]).toMatchObject({ option: 2, frame: "b", label: "Warmer" });
    const turns = buildTurns(
      [
        proposed,
        lanes,
        event(
          7,
          {
            type: "step",
            tool: "read_skill",
            arguments: "{}",
            result: "{}",
            startedAt: 0,
            durationMs: 1,
            outcome: "done",
            detail: {},
          },
          "agent2",
        ),
      ],
      {},
    );
    expect(turns[0]?.orchestration.lanes[0]?.items).toHaveLength(0);
    expect(turns[0]?.orchestration.lanes[1]?.items).toHaveLength(1);
  });
  it("routes Build and Keep to the captured session, waits for daemon records and exposes errors", async () => {
    const feed = new FakeAgentFeed(
      [
        {
          id: "session",
          kind: { type: "engine", engine: "builtin" },
          agent: "agent1",
          title: "Screens",
          createdAt: 0,
          updatedAt: 0,
          state: "awaiting_plan",
          agents: 1,
        },
      ],
      { session: [proposed] },
    );
    const c = createAgentController({ feed, now: () => 0 });
    await c.load();
    await c.buildPlan("turn", plan, { count: 4, mode: "split" });
    expect(feed.requests.at(-1)).toMatchObject({
      type: "buildPlan",
      session: "session",
      turn: "turn",
      parallel: { count: 4, mode: "split" },
    });
    expect(c.turns()[0]?.orchestration.status).toBe("proposed");
    await c.keepOptions("turn", [2]);
    expect(feed.requests.at(-1)).toMatchObject({ type: "keepOptions", options: [2] });
    feed.fail = "Offline";
    await c.editRequest("turn", "Changed request");
    expect(c.state.error).toBe("Offline");
    c.dispose();
  });
});
it("live lane steps keep their agent when another lane finishes the same tool", async () => {
  const feed = new FakeAgentFeed(
    [
      {
        id: "session",
        kind: { type: "engine", engine: "builtin" },
        agent: "agent1",
        title: "Screens",
        createdAt: 0,
        updatedAt: 0,
        state: "running",
        agents: 2,
      },
    ],
    { session: [proposed, lanes] },
  );
  const c = createAgentController({ feed, now: () => 10 });
  await c.load();
  feed.emit({
    type: "live",
    session: "session",
    turn: "turn",
    agent: "agent2",
    item: { type: "toolStarted", call: "two", name: "read_skill" },
  });
  expect(c.turns()[0]?.orchestration.lanes[1]?.items).toHaveLength(1);
  expect(c.turns()[0]?.orchestration.lanes[0]?.items).toHaveLength(0);
  c.dispose();
});
it("unknown or truncated plan and parallel records fail at the codec boundary", async () => {
  const { parseEvent } = await import("./log");
  expect(() =>
    parseEvent(event(1, { type: "plan", event: "proposed", plan: { title: "Incomplete" } })),
  ).toThrow();
  expect(() =>
    parseEvent(event(1, { type: "parallel", event: "laneEnded", agent: "agent1", end: "maybe" })),
  ).toThrow();
  expect(() => parseEvent(event(1, { type: "parallel", event: "newer_mode" }))).toThrow();
});
it("panel and canvas alternative choices share one turn-scoped selection", async () => {
  const feed = new FakeAgentFeed([]);
  const c = createAgentController({ feed, now: () => 0 });
  c.chooseOption("first", 2);
  c.chooseOption("second", 3);
  expect(c.state.selectedOptions).toEqual({ first: [2], second: [3] });
  c.dispose();
});
