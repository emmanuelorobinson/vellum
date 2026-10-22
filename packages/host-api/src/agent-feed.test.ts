import { describe, expect, it } from "vitest";
import { agentFeedRequest } from "./agent-feed";

describe("agent feed requests", () => {
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
