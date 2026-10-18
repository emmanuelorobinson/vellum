// TEST DATA: C04, C05, C07 and C35 as the existing session codec and presence port report them.
import {
  type AgentPlan,
  FakeAgentFeed,
  FakePresenceFeed,
  type FeedUpdate,
  type PresenceSnapshot,
  type SessionEvent,
} from "@vellum/editor-core";
import type { Node } from "@vellum/types-ts";
import { AGENT_NOW, AGENTS, SESSIONS } from "./agent-scenarios";

const T = "01J00000000000000000000T01",
  S = SESSIONS.onboarding,
  MODEL = "deepseek/deepseek-v4.1-flash";
const groups = [
  {
    page: "p1",
    name: "Onboarding",
    names: ["Welcome", "Step 1", "Step 2", "Step 3"],
    agent: AGENTS.teal,
  },
  {
    page: "p2",
    name: "Settings",
    names: ["Profile", "Preferences", "Notifications"],
    agent: AGENTS.saffron,
  },
  { page: "p3", name: "Paywall", names: ["Plans", "Checkout"], agent: AGENTS.rose },
  {
    page: "p4",
    name: "Components",
    names: ["Button", "Habit card", "Progress"],
    agent: AGENTS.green,
  },
];
export const PLAN: AgentPlan = {
  title: "12 screens from DESIGN.md",
  items: groups.flatMap((g, i) =>
    g.names.map((name, n) => ({
      id: i * 4 + n + 1,
      name,
      page: g.page,
      intent: `Build ${name}`,
      width: 184,
      height: 100,
    })),
  ),
};
export const COLLAB_NAMES = Object.fromEntries(groups.map((g) => [g.page, g.name]));
const node = (
  type: string,
  id: string,
  name: string,
  x: number,
  y: number,
  width: number,
  height: number,
  extra = {},
) =>
  ({
    type,
    id,
    name,
    position: { type: "absolute", x, y },
    width,
    height,
    ...extra,
  }) as unknown as Node;
export function collaborationNodes(name: string): Node[] {
  if (name === "c35")
    return [
      node("note", "note", "For Lena", 456, 84, 282, 82, { content: "Check copy with Lena" }),
      node("context", "rules", "Onboarding rules", 754, 84, 282, 150, {
        content: "All onboarding screens: one primary action, max 2 lines of body text",
        pointsAt: "step2",
      }),
      node("prompt", "prompt", "Tablet version", 950, 396, 282, 177, {
        content: "Make a tablet version of Step 2",
        engine: "builtin",
        model: MODEL,
      }),
      node("frame", "step2", "Step 2", 600, 420, 180, 260),
    ];
  if (name.startsWith("c07"))
    return Array.from({ length: name === "c07e" ? 4 : 3 }, (_, i) =>
      node(
        "frame",
        `option${i + 1}`,
        String.fromCharCode(65 + i),
        440 + i * (name === "c07e" ? 207 : 277),
        150,
        name === "c07e" ? 195 : 261,
        520,
      ),
    );
  return name === "c05"
    ? groups.flatMap((g, row) =>
        g.names.flatMap((n, col) =>
          col < (row === 0 ? 2 : 1)
            ? [
                node(
                  "frame",
                  `screen${row * 4 + col + 1}`,
                  n,
                  440 + col * 204,
                  129 + row * 161,
                  184,
                  100,
                ),
              ]
            : [],
        ),
      )
    : [];
}
export function collaborationScenario(name: string) {
  const events: SessionEvent[] = [];
  const ev = (body: unknown, agent: string = AGENTS.teal) => {
    const e = {
      seq: events.length + 1,
      at: AGENT_NOW - 1000,
      origin: { actor: { kind: "agent", agent }, session: S, turn: T },
      body,
    } as SessionEvent;
    events.push(e);
    return e;
  };
  const snapshot: PresenceSnapshot = {
    connected: true,
    paused: false,
    agents: [],
    leases: [],
    highlights: [],
  };
  const state = name === "c04" ? "awaiting_plan" : name === "c05" ? "running" : "done";
  ev({
    type: "prompt",
    text:
      name === "c04"
        ? "Design all screens in @DESIGN.md."
        : name === "c05"
          ? "Build all screens in @DESIGN.md."
          : name === "c35"
            ? "Refine Step 2 using the onboarding rules."
            : "Design a paywall. Explore three options.",
    context: [],
    images: [],
  });
  ev({ type: "turn", turn: T, from: "planning", input: "no_plan_needed", to: state });
  if (name === "c04") ev({ type: "plan", event: "proposed", plan: PLAN });
  if (name === "c05") {
    const placeholders = PLAN.items.map((item) => {
      const row = groups.findIndex((g) => g.page === item.page);
      const group = groups[row]!;
      const col = group.names.indexOf(item.name);
      return {
        item: item.id,
        frame: `screen${item.id}`,
        name: item.name,
        page: "p1",
        bounds: { x: 440 + col * 204, y: 129 + row * 161, width: 184, height: 100 },
        agent: group.agent,
        turn: T,
      };
    });
    ev({ type: "plan", event: "built", plan: PLAN, placeholders });
    ev({ type: "plan", event: "ticked", items: [1, 2, 5, 9, 13] });
    ev({
      type: "parallel",
      event: "lanes",
      mode: "split",
      lanes: groups.map((g) => ({
        agent: g.agent,
        name:
          g.agent === AGENTS.teal
            ? "Teal"
            : g.agent === AGENTS.saffron
              ? "Saffron"
              : g.agent === AGENTS.rose
                ? "Rose"
                : "Green",
        model: MODEL,
        items: PLAN.items.filter((i) => i.page === g.page).map((i) => i.id),
      })),
    });
    groups.forEach((g, row) => {
      ev(
        {
          type: "step",
          tool: "insert_nodes",
          arguments: "{}",
          result: "{}",
          startedAt: AGENT_NOW - 5000,
          durationMs: 4000,
          outcome: "done",
          detail: {
            verb: "designed",
            target: { id: `screen${row * 4 + 1}`, name: g.names[0], type: "frame" },
            counts: { created: 28, changed: 0, removed: 0, variables: 0 },
          },
        },
        g.agent,
      );
      snapshot.agents.push({
        id: g.agent,
        name: ["Teal", "Saffron", "Rose", "Green"][row]!,
        hue: ["teal", "saffron", "rose", "green"][row]!,
        session: S,
        state: "working",
        task: `Building ${g.names[1]}`,
        cursor: { x: 760, y: 180 + row * 161 },
        viewport: { x: 0, y: 0, zoom: 1 },
      });
      snapshot.leases.push({
        frame: `screen${row * 4 + 2}`,
        agent: g.agent,
        page: "p1",
        bounds: { x: 644, y: 129 + row * 161, width: 184, height: 100 },
        changes: 14,
        rejected: row === 1 ? 2 : 0,
        editing: false,
      });
    });
  }
  if (name.startsWith("c07")) {
    ev({
      type: "parallel",
      event: "lanes",
      mode: name === "c07e" ? "explore_style" : "side_by_side",
      lanes: [AGENTS.teal, AGENTS.saffron, AGENTS.rose].map((agent, i) => ({
        agent,
        name: ["Teal", "Saffron", "Rose"][i],
        model: MODEL,
        items: [],
      })),
    });
    for (let i = 1; i <= (name === "c07e" ? 4 : 3); i++)
      ev({
        type: "parallel",
        event: "option",
        option: i,
        agent: [AGENTS.teal, AGENTS.saffron, AGENTS.rose][(i - 1) % 3],
        frame: `option${i}`,
        ...(name === "c07e"
          ? {
              label: [
                "Original",
                "Warmer palette, serif headings",
                "Dark, high contrast",
                "Light, spacious",
              ][i - 1],
            }
          : {}),
      });
  }
  if (name === "c35")
    snapshot.agents.push({ id: AGENTS.teal, name: "Teal", hue: "teal", session: S, state: "idle" });
  return {
    feed: new FakeAgentFeed(
      [
        {
          id: S,
          kind: { type: "engine", engine: "builtin" },
          agent: AGENTS.teal,
          title: name.startsWith("c07") ? "Paywall variants" : "All screens from DESIGN.md",
          createdAt: AGENT_NOW - 10000,
          updatedAt: AGENT_NOW,
          state,
          agents: name === "c05" ? 4 : 1,
          model: MODEL,
        },
      ],
      { [S]: events },
    ),
    updates: [] as FeedUpdate[],
    presenceFeed: new FakePresenceFeed(snapshot),
    snapshot,
  };
}
