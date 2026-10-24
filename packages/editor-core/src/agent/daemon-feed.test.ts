// TEST DATA: the daemon feed over a scripted `vellum/agentFeed`.
import { describe, expect, it } from "vitest";
import { DaemonAgentFeed } from "./daemon-feed";

const S = "01JA00000000000000000SESS1" as never;
function daemon() {
  const calls: [string, Record<string, unknown>][] = [];
  let events = [
    { seq: 1, at: 1, origin: { actor: { kind: "agent", agent: "a" } }, body: { type: "message" } },
  ];
  const call = async (action: string, params: Record<string, unknown>) => {
    calls.push([action, params]);
    if (action === "sessions") return [{ id: S, title: "Claude Code", updatedAt: events.length }];
    if (action === "events") return events.filter((e) => e.seq > Number(params.since));
    if (action === "image") return { mime: "image/png", base64: "AAAA" };
    return { done: true };
  };
  return {
    calls,
    call,
    add: () => (events = [...events, { ...events[0]!, seq: events.length + 1 }]),
  };
}

describe("DaemonAgentFeed", () => {
  it("reads sessions and events, then streams only what is new", async () => {
    const d = daemon();
    const feed = new DaemonAgentFeed(d.call, { device: "D" as never, interval: 1e9 });
    const updates: string[] = [];
    const sub = feed.subscribe((u) =>
      updates.push(u.type === "event" ? `event ${u.event.seq}` : u.type),
    );
    expect((await feed.events(S)).map((e) => e.seq)).toEqual([1]);
    await feed.poll();
    d.add();
    await feed.poll();
    await feed.poll();
    expect(updates).toEqual(["sessions", "sessions", "event 2"]);
    sub.dispose();
  });

  it("controls go to the daemon with the person's device", async () => {
    const d = daemon();
    const feed = new DaemonAgentFeed(d.call, { device: "D" as never });
    await feed.undoTurn(S, null);
    await feed.stop(S);
    expect(d.calls.map(([a]) => a)).toEqual(["undoTurn", "stop"]);
    expect(d.calls[0]?.[1]).toEqual({ session: S, device: "D" });
  });

  it("creates sessions and sends the complete composer request through the host", async () => {
    const request = {
      text: "TEST DATA design",
      context: [],
      skills: [],
      parallel: { count: 1, mode: "split" as const },
      explore: null,
      engine: { engine: "codex", model: "default" },
      queue: null,
    };
    const call = async (action: string, params: Record<string, unknown>) => {
      if (action === "create") return { id: S };
      if (action === "send") {
        expect(params).toEqual({ session: S, request });
        return { turn: "T" };
      }
      if (action === "engines")
        return { builtin: [], installed: [{ id: "codex", name: "Codex", state: "signed_in" }] };
      return [];
    };
    const feed = new DaemonAgentFeed(call, { device: "D" as never });
    expect((await feed.create()).id).toBe(S);
    expect(await feed.send(S, request)).toEqual({ turn: "T" });
    expect((await feed.engines()).installed[0]?.state).toBe("signed_in");
  });

  it("fetches a step's picture once and returns it as a data URL", async () => {
    const d = daemon();
    const feed = new DaemonAgentFeed(d.call, { device: "D" as never });
    expect(feed.image("abc")).toBeNull();
    expect(feed.image("abc")).toBeNull();
    await Promise.resolve();
    await Promise.resolve();
    expect(feed.image("abc")).toBe("data:image/png;base64,AAAA");
    expect(d.calls.filter(([a]) => a === "image")).toHaveLength(1);
  });

  it("retries a failed request against the existing design instead of replaying raw writes", async () => {
    let sent: unknown;
    const feed = new DaemonAgentFeed(
      async (action, params) => {
        if (action === "events")
          return [
            { seq: 1, origin: { turn: "T" }, body: { type: "prompt", text: "TEST DATA original" } },
          ];
        if (action === "send") {
          sent = params.request;
          return { turn: "NEXT" };
        }
        return [];
      },
      { device: "D" as never },
    );
    await feed.retryStep(S, "T" as never, 4);
    expect(sent).toMatchObject({ text: expect.stringContaining("Repair failed step 4") });
    expect(sent).toMatchObject({ text: expect.stringContaining("TEST DATA original") });
  });
});
