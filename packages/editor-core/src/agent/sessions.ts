/** The session switcher (DESIGN.md "Session bar"): every session of this document, grouped and newest first. */
import { guardReasons } from "@vellum/config";
import type { AgentId, SessionId } from "@vellum/types-ts";
import type { TurnStateName } from "./log";

export interface SessionInfo {
  id: SessionId;
  kind: { type: "engine"; engine: string } | { type: "external"; client: string };
  /** The agent whose work it records: a built-in hue or an external client. */
  agent: AgentId;
  /** Generated from the first request, renameable. */
  title: string;
  createdAt: number;
  updatedAt: number;
  /** Its newest turn's state; `null` before the first turn (and for external agents, which have none). */
  state: TurnStateName | null;
  /** How many agents its running turn has (lanes). */
  agents: number;
  /** What a turn that needs you waits for, beyond its state: alternatives waiting for Keep. */
  waiting?: "choice";
  /** External agents: connected over MCP now. */
  connected?: boolean;
  /** The model the session's engine uses, before any turn has reported usage. */
  model?: string;
}

export type SessionGroup = "Running" | "Needs you" | "Recent";
export type SessionDot = "working" | "needs-you" | "done" | "failed";

/** As vellum-core `TurnState::dot`; a waiting choice and an external connection count too. */
export function sessionDot(s: SessionInfo): SessionDot {
  if (s.waiting || s.state === "awaiting_plan" || s.state === "awaiting_permission")
    return "needs-you";
  if (s.state === "queued" || s.state === "planning" || s.state === "running") return "working";
  if (s.kind.type === "external" && s.connected) return "working";
  return s.state === "failed" ? "failed" : "done";
}

export function groupSessions(
  sessions: readonly SessionInfo[],
): { group: SessionGroup; sessions: SessionInfo[] }[] {
  const of = (s: SessionInfo): SessionGroup => {
    const dot = sessionDot(s);
    return dot === "working" ? "Running" : dot === "needs-you" ? "Needs you" : "Recent";
  };
  const sorted = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt);
  return (["Running", "Needs you", "Recent"] as const)
    .map((group) => ({ group, sessions: sorted.filter((s) => of(s) === group) }))
    .filter((g) => g.sessions.length > 0);
}

export interface TimeOptions {
  now: number;
  /** An IANA zone; the computer's own when absent. */
  timeZone?: string;
}

/** "today, 10:12", "yesterday, 09:05", "Oct 2, 09:05". */
export function when(at: number, { now, timeZone }: TimeOptions): string {
  const day = new Intl.DateTimeFormat("en-CA", {
    timeZone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
  const time = new Intl.DateTimeFormat("en-GB", {
    timeZone,
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(at);
  if (day.format(at) === day.format(now)) return `today, ${time}`;
  if (day.format(at) === day.format(now - 86_400_000)) return `yesterday, ${time}`;
  const date = new Intl.DateTimeFormat("en-US", {
    timeZone,
    month: "short",
    day: "numeric",
  }).format(at);
  return `${date}, ${time}`;
}

/** "8 minutes", "2 hours"; `short` gives "8 min", "2 h" for the header. */
export function connectedFor(ms: number, short = false): string {
  const minutes = Math.max(1, Math.round(ms / 60_000));
  if (minutes < 60)
    return short ? `${minutes} min` : `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
  const hours = Math.round(minutes / 60);
  return short ? `${hours} h` : `${hours} ${hours === 1 ? "hour" : "hours"}`;
}

/** A switcher row's second line. */
export function sessionLine(s: SessionInfo, time: TimeOptions): string {
  if (s.kind.type === "external")
    return s.connected
      ? `external · ${connectedFor(time.now - s.createdAt)}`
      : `external · ${when(s.updatedAt, time)}`;
  if (s.waiting === "choice") return "Choose a variant";
  switch (s.state) {
    case "awaiting_plan":
      return "Waiting for plan approval";
    case "awaiting_permission":
      return "Waiting for permission";
    case "queued":
    case "planning":
    case "running":
      return s.agents > 1 ? `${s.agents} agents · working` : "working";
    case "done":
      return `Done · ${when(s.updatedAt, time)}`;
    case "failed":
      return `Failed · ${when(s.updatedAt, time)}`;
    case "stopped":
      return `${guardReasons.stopped} · ${when(s.updatedAt, time)}`;
    default:
      return `New · ${when(s.createdAt, time)}`;
  }
}

/** The session bar's title: an external agent's says so ("Claude Code · external"). */
export const sessionTitle = (s: SessionInfo) =>
  s.kind.type === "external" ? `${s.kind.client} · external` : s.title;
