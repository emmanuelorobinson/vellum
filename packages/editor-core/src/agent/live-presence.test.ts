// TEST DATA. A scene, a clock and timers driven by hand; no daemon, engine or document store.

import { expect, it } from "vitest";
import { createLivePresence, STEP_MS } from "./live-presence";
import type { PresenceSnapshot } from "./presence";

type Item = {
  node: Record<string, unknown>;
  bounds: { x: number; y: number; width: number; height: number };
  children: string[];
  parent: string | null;
};
function setup() {
  const items = new Map<string, Item>();
  const put = (
    id: string,
    parent: string | null,
    x: number,
    y: number,
    w: number,
    h: number,
    name = id,
  ) => {
    items.set(id, {
      node: { id, name },
      bounds: { x, y, width: w, height: h },
      children: [],
      parent,
    });
    if (parent) items.get(parent)?.children.push(id);
  };
  put("frame", null, 0, 0, 400, 800);
  const docListeners = new Set<() => void>();
  const turnListeners = new Set<() => void>();
  let state: "running" | "done" = "done";
  const timers: { at: number; fn: () => void }[] = [];
  let now = 1000;
  const advance = (ms: number) => {
    const end = now + ms;
    for (;;) {
      timers.sort((a, b) => a.at - b.at);
      const next = timers[0];
      if (!next || next.at > end) break;
      timers.shift();
      now = next.at;
      next.fn();
    }
    now = end;
  };
  const presence = createLivePresence({
    agent: {
      state: { current: "session" },
      turns: () => [{ id: "t1", external: false, state, agents: ["agent1"], items: [] }] as never,
      subscribe: (l) => (turnListeners.add(l), { dispose: () => void turnListeners.delete(l) }),
    },
    client: {
      scene: {
        roots: () => [...items.values()].filter((i) => !i.parent).map((i) => i.node.id as string),
        get: (id: string) => items.get(id),
      } as never,
      subscribe: ((l: () => void) => {
        docListeners.add(l);
        return { dispose: () => void docListeners.delete(l) };
      }) as never,
    },
    now: () => now,
    page: () => "page" as never,
    engineName: () => "Codex",
    engineId: () => "codex",
    connected: () => true,
    timers: {
      set: (fn, ms) => {
        const t = { at: now + ms, fn };
        timers.push(t);
        return t;
      },
      clear: (h) => void timers.splice(timers.indexOf(h as never), 1),
    },
  });
  const seen: PresenceSnapshot[] = [];
  presence.feed.subscribe((s) => seen.push(s));
  return {
    presence,
    seen,
    put,
    advance,
    start() {
      state = "running";
      for (const l of [...turnListeners]) l();
    },
    finish() {
      state = "done";
      for (const l of [...turnListeners]) l();
    },
    edited() {
      for (const l of [...docListeners]) l();
    },
    last: () => seen.at(-1) as PresenceSnapshot,
  };
}

it("shows nothing while no turn runs, and the engine's name and hue once one does", async () => {
  const t = setup();
  expect((await t.presence.feed.read()).agents).toEqual([]);
  t.start();
  const agent = t.last().agents[0];
  expect(agent).toMatchObject({ id: "agent1", name: "Codex", state: "working" });
  expect(agent?.cursor).toBeUndefined();
  expect(t.presence.actors.agent1).toMatchObject({ kind: "agent", name: "Codex", hue: agent?.hue });
});

it("walks the cursor over the nodes the agent adds, biggest first, and leases their frame", () => {
  const t = setup();
  t.start();
  t.put("card", "frame", 20, 20, 360, 200);
  t.put("title", "card", 40, 40, 100, 20);
  t.edited();
  t.advance(100);
  // The first stop is taken at once; the rest follow one step apart.
  expect(t.last().agents[0]?.cursor).toEqual({ x: 200, y: 120 });
  t.advance(STEP_MS);
  expect(t.last().agents[0]?.cursor).toEqual({ x: 90, y: 50 });
  expect(t.last().leases).toHaveLength(1);
  expect(t.last().leases[0]).toMatchObject({ frame: "frame", agent: "agent1", idle: false });
});

it("removes the cursor when the turn ends and lets the lease go a moment later", () => {
  const t = setup();
  t.start();
  t.put("card", "frame", 20, 20, 360, 200);
  t.edited();
  t.advance(100);
  t.finish();
  expect(t.last().agents).toEqual([]);
  expect(t.last().leases[0]?.idle).toBe(true);
  t.advance(3000);
  expect(t.last().leases).toEqual([]);
});
