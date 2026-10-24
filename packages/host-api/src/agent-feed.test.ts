import { describe, expect, it } from "vitest";
import { agentFeedRequest } from "./agent-feed";

describe("agent feed requests", () => {
  it("accepts a bounded prompt and rejects process controls and malformed engine choices", () => {
    const request = {
      text: "TEST DATA design",
      context: [],
      skills: [],
      parallel: { count: 1, mode: "split" },
      explore: null,
      engine: { engine: "codex", model: "default" },
      queue: null,
    };
    expect(agentFeedRequest("send", { session: "01JA", request })).not.toBeNull();
    for (const invalid of [
      { ...request, command: "/bin/sh" },
      { ...request, text: "" },
      { ...request, parallel: { count: 7, mode: "split" } },
      { ...request, engine: { engine: "sh", model: "default" } },
      { ...request, engine: { engine: "codex", model: "--config secret" } },
    ])
      expect(agentFeedRequest("send", { session: "01JA", request: invalid })).toBeNull();
    expect(agentFeedRequest("create", { path: "/outside.vellum" })).toBeNull();
  });
  it("allows only each action's own fields, with ids and numbers", () => {
    expect(agentFeedRequest("sessions", {})).toEqual({ action: "sessions", params: {} });
    expect(agentFeedRequest("events", { session: "01JA", since: 3 })).toEqual({
      action: "events",
      params: { session: "01JA", since: 3 },
    });
    expect(agentFeedRequest("undoTurn", { session: "01JA" })).not.toBeNull();
    for (const [action, params] of [
      ["events", { session: "01JA" }],
      ["events", { session: "01JA", since: -1 }],
      ["remove", { session: "../x" }],
      ["sessions", { path: "/etc" }],
      ["undoTurn", { session: "01JA", device: "someone" }],
      ["send", {}],
    ] as const)
      expect(agentFeedRequest(action, params)).toBeNull();
  });
});
