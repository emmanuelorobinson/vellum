// TEST DATA. No network, clock or document store behind this port.

import type { ToolTransaction } from "@vellum/engine-client";
import { FakeEngineClient } from "@vellum/engine-client";
import { expect, it } from "vitest";
import { createPresenceController, FakePresenceFeed, leaseLabel } from "./presence";

const data = () => ({
  connected: true,
  paused: false,
  agents: [
    {
      id: "a",
      name: "Agent A",
      hue: "teal",
      session: "s",
      state: "working" as const,
      task: "Building Start",
      cursor: { x: 10, y: 20 },
      viewport: { x: 1, y: 2, zoom: 1 },
    },
  ],
  leases: [
    {
      frame: "f",
      agent: "a",
      page: "p",
      bounds: { x: 0, y: 0, width: 200, height: 300 },
      changes: 14,
      rejected: 2,
      editing: false,
    },
  ],
  highlights: [],
});
it("labels every lease state; rejects colour-only identity", () => {
  expect(leaseLabel({ ...data().leases[0]!, editing: true })).toContain("editing here too");
  expect(leaseLabel(data().leases[0]!)).toContain("2 changes rejected");
});
it("person commits inside a lease are accepted and update the tab, rejected commits do not", async () => {
  const feed = new FakePresenceFeed(data());
  const engine = new FakeEngineClient({ revision: 0, document: null, readOnly: false });
  const c = createPresenceController({
    feed,
    engine,
    now: () => 100,
    onFollow: () => {},
    contains: (frame, node) => frame === "f" && node === "child",
  });
  await c.load();
  const tx: ToolTransaction = {
    origin: { actor: { kind: "person", device: "d" } },
    ops: [{ type: "update", id: "child", patch: { name: "Edited" } }],
  };
  expect(engine.apply(tx).status).toBe("rejected");
  expect(c.leases()[0]?.editing).toBe(false);
  engine.script({ revision: 1, document: null, readOnly: false });
  expect(engine.apply(tx).status).toBe("applied");
  expect(c.leases()[0]?.editing).toBe(true);
  expect(leaseLabel(c.leases()[0]!)).toContain("editing here too");
  c.dispose();
});
it("Stop blocks a frame through the feed, Pause waits for ack, follow stops on disconnect", async () => {
  const feed = new FakePresenceFeed(data());
  let camera: unknown = null;
  const c = createPresenceController({
    feed,
    now: () => 0,
    onFollow: (v) => {
      camera = v;
    },
  });
  await c.load();
  c.follow("a");
  expect(camera).toEqual({ x: 1, y: 2, zoom: 1 });
  await c.action({ type: "stopFrame", agent: "a", frame: "f" });
  expect(feed.requests.at(-1)).toMatchObject({ type: "stopFrame", frame: "f" });
  await c.action({ type: "pauseAll" });
  expect(c.state.snapshot.paused).toBe(false);
  feed.emit({ ...data(), paused: true });
  expect(c.state.snapshot.paused).toBe(true);
  feed.emit({ ...data(), connected: false });
  expect(c.state.following).toBe(null);
  expect(c.agents()).toEqual([]);
  c.dispose();
});
