/** What a webview or renderer may ask of the daemon's agent feed (`vellum/agentFeed`); the host adds the path
 * and the person's device. Anything else is refused before it reaches the daemon. */
export type AgentFeedAction = "sessions" | "events" | "image" | "remove" | "undoTurn" | "stop";
const FIELDS: Record<AgentFeedAction, Record<string, "string" | "number" | "optional-string">> = {
  sessions: {},
  events: { session: "string", since: "number" },
  image: { hash: "string" },
  remove: { session: "string" },
  undoTurn: { session: "string", turn: "optional-string" },
  stop: { session: "string" },
};

export function agentFeedRequest(
  action: unknown,
  params: unknown,
): { action: AgentFeedAction; params: Record<string, string | number> } | null {
  if (typeof action !== "string" || !(action in FIELDS)) return null;
  if (!params || typeof params !== "object" || Array.isArray(params)) return null;
  const fields = FIELDS[action as AgentFeedAction];
  const entries = Object.entries(params as Record<string, unknown>);
  for (const [key, value] of entries) {
    const kind = fields[key];
    if (!kind) return null;
    if (
      kind === "number"
        ? !Number.isSafeInteger(value) || (value as number) < 0
        : typeof value !== "string"
    )
      return null;
    if (typeof value === "string" && !/^[A-Za-z0-9]{1,80}$/.test(value)) return null;
  }
  for (const [key, kind] of Object.entries(fields))
    if (kind !== "optional-string" && !(key in (params as object))) return null;
  return {
    action: action as AgentFeedAction,
    params: Object.fromEntries(entries) as Record<string, string | number>,
  };
}
