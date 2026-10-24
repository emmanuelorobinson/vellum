/** What a webview or renderer may ask of the daemon's agent feed (`vellum/agentFeed`); the host adds the path
 * and the person's device. Anything else is refused before it reaches the daemon. */
export type AgentFeedAction =
  | "sessions"
  | "events"
  | "image"
  | "remove"
  | "undoTurn"
  | "stop"
  | "create"
  | "send"
  | "engines";
const FIELDS: Record<AgentFeedAction, Record<string, "string" | "number" | "optional-string">> = {
  sessions: {},
  events: { session: "string", since: "number" },
  image: { hash: "string" },
  remove: { session: "string" },
  undoTurn: { session: "string", turn: "optional-string" },
  stop: { session: "string" },
  create: {},
  engines: {},
  send: { session: "string" },
};

export function agentFeedRequest(
  action: unknown,
  params: unknown,
): { action: AgentFeedAction; params: Record<string, unknown> } | null {
  if (typeof action !== "string" || !(action in FIELDS)) return null;
  if (!params || typeof params !== "object" || Array.isArray(params)) return null;
  if (action === "send") {
    const p = params as Record<string, unknown>;
    if (
      Object.keys(p).sort().join(",") !== "request,session" ||
      typeof p.session !== "string" ||
      !/^[A-Za-z0-9]{1,80}$/.test(p.session)
    )
      return null;
    const request = promptRequest(p.request);
    return request ? { action, params: { session: p.session, request } } : null;
  }
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

export interface NativePrompt {
  text: string;
  engine: {
    engine: "codex" | "claude-code";
    model: string;
    effort?: "low" | "medium" | "high";
  } | null;
  context: { kind: string; label: string; value: string }[];
  skills: string[];
  parallel: { count: number; mode: "split" | "side_by_side" };
  explore: { count: number; vary: "layout" | "style" } | null;
  queue: "after" | "now" | null;
}
/** Renderer input is data, never process arguments, paths or executable names. */
export function promptRequest(value: unknown): NativePrompt | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const r = value as NativePrompt;
  if (
    Object.keys(r).some(
      (k) =>
        ![
          "text",
          "engine",
          "context",
          "skills",
          "parallel",
          "explore",
          "queue",
          "sourceNode",
        ].includes(k),
    )
  )
    return null;
  if (
    typeof r.text !== "string" ||
    !r.text.trim() ||
    r.text.length > 100_000 ||
    !Array.isArray(r.context) ||
    r.context.length > 32 ||
    !r.context.every(
      (c) =>
        c &&
        typeof c.kind === "string" &&
        typeof c.label === "string" &&
        typeof c.value === "string" &&
        c.value.length < 4096,
    )
  )
    return null;
  if (
    !Array.isArray(r.skills) ||
    r.skills.length > 32 ||
    !r.skills.every((s) => typeof s === "string" && s.length < 1024)
  )
    return null;
  if (
    !r.parallel ||
    !Number.isInteger(r.parallel.count) ||
    r.parallel.count < 1 ||
    r.parallel.count > 6 ||
    !["split", "side_by_side"].includes(r.parallel.mode)
  )
    return null;
  if (
    r.explore !== null &&
    (!r.explore ||
      !Number.isInteger(r.explore.count) ||
      r.explore.count < 2 ||
      r.explore.count > 6 ||
      !["layout", "style"].includes(r.explore.vary))
  )
    return null;
  if (r.queue !== null && !["after", "now"].includes(r.queue)) return null;
  if (
    r.engine !== null &&
    (!r.engine ||
      !["codex", "claude-code"].includes(r.engine.engine) ||
      typeof r.engine.model !== "string" ||
      !/^[A-Za-z0-9._:/-]{1,120}$/.test(r.engine.model) ||
      (r.engine.effort !== undefined && !["low", "medium", "high"].includes(r.engine.effort)))
  )
    return null;
  return r;
}
