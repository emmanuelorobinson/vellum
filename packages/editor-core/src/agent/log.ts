/**
 * A session's events as the daemon's session store keeps them (vellum-core `sessions::codec`, ADR-006): the same
 * camelCase JSON, read here and never written. Reading refuses anything this version does not know, as the Rust
 * codec does, so a newer daemon cannot be misread as an older one.
 */
import type { AgentId, Origin, TurnId } from "@vellum/types-ts";

export type TurnStateName =
  | "queued"
  | "planning"
  | "awaiting_plan"
  | "running"
  | "awaiting_permission"
  | "done"
  | "failed"
  | "stopped";
export const TERMINAL: readonly TurnStateName[] = ["done", "failed", "stopped"];

/** What a step did, as `vellum::tools::step::Verb` names it. */
export type Verb =
  | "read"
  | "opened"
  | "checked_layout"
  | "checked_screenshot"
  | "designed"
  | "added_asset"
  | "updated"
  | "copied"
  | "moved"
  | "replaced"
  | "deleted"
  | "updated_variables"
  | "made_component"
  | "made_slot"
  | "claimed"
  | "released"
  | "ran_script"
  | "progress";

/** The step descriptor the daemon stores as a step's `detail` (`vellum::tools::step::Step`). */
export interface StepDetail {
  tool?: string;
  verb?: Verb;
  outcome?: "done" | "failed";
  document?: string;
  target?: { id: string; name: string; type: string };
  counts?: { created: number; changed: number; removed: number; variables: number };
  thumbnail?: { node: string; scale: number };
  script?: string;
  message?: string;
  progress?: [number, number];
  items?: number[];
  error?: string;
  startedAt?: number;
  durationMs?: number;
}

export type StoredPart =
  | { text: string }
  | { image: { mime: string; hash: string } }
  | { toolCall: { id: string; name: string; arguments: unknown; metadata: unknown } }
  | { toolResult: { id: string; name: string; result: unknown; failed: boolean } }
  | { continuation: unknown };

export type StepBody = {
  type: "step";
  tool: string;
  /** Compact JSON, cut with a marker. */
  arguments: string;
  result: string;
  startedAt: number;
  durationMs: number;
  outcome: "done" | "failed" | "rejected";
  detail: StepDetail;
  /** A cached picture's SHA-256: the frame after a designed step, or the screenshot a check looked at. */
  thumbnail?: string;
};

export type EventBody =
  | { type: "prompt"; text: string; context: string[]; images: string[] }
  | { type: "messages"; messages: { role: "user" | "assistant"; parts: StoredPart[] }[] }
  | StepBody
  | {
      type: "usage";
      model: string;
      tokens: { input: number; output: number; cacheRead: number; cacheWrite: number };
      /** Micro-USD at API rates; always an estimate. */
      estimate?: number;
    }
  | { type: "turn"; turn: TurnId; from: TurnStateName; input: string; to: TurnStateName }
  | {
      type: "permission";
      action: Record<string, unknown>;
      kind: string;
      answer: string;
      source: Record<string, unknown>;
      engine?: string;
    }
  | { type: "plan"; event: string; [key: string]: unknown }
  | {
      type: "parallel";
      event: "lanes" | "laneEnded" | "option";
      lanes?: { agent: AgentId; [key: string]: unknown }[];
      [key: string]: unknown;
    }
  | { type: "undo"; changed: number; kept: number };

export interface SessionEvent {
  /** Its place in the session, from 1. */
  seq: number;
  /** UTC milliseconds. */
  at: number;
  origin: Origin;
  body: EventBody;
}

const KINDS = [
  "prompt",
  "messages",
  "step",
  "usage",
  "turn",
  "permission",
  "plan",
  "parallel",
  "undo",
] as const;
const object = (v: unknown, at: string): Record<string, unknown> => {
  if (!v || typeof v !== "object" || Array.isArray(v)) throw Error(`${at} is not an object`);
  return v as Record<string, unknown>;
};
const number = (o: Record<string, unknown>, key: string, at: string): number => {
  const v = o[key];
  if (typeof v !== "number" || !Number.isFinite(v)) throw Error(`${at}.${key} is not a number`);
  return v;
};
const string = (o: Record<string, unknown>, key: string, at: string): string => {
  const v = o[key];
  if (typeof v !== "string") throw Error(`${at}.${key} is not text`);
  return v;
};

/** Checks the fields the panel reads; the rest is the daemon's business and passes through unchanged. */
export function parseEvent(value: unknown): SessionEvent {
  const e = object(value, "event");
  const origin = object(e.origin, "event.origin");
  object(origin.actor, "event.origin.actor");
  const body = object(e.body, "event.body");
  const type = string(body, "type", "event.body");
  if (!(KINDS as readonly string[]).includes(type))
    throw Error(`event.body.type "${type}" is not one this version reads`);
  if (type === "prompt") string(body, "text", "prompt");
  if (type === "step") {
    string(body, "tool", "step");
    number(body, "durationMs", "step");
    object(body.detail, "step.detail");
    if (!["done", "failed", "rejected"].includes(string(body, "outcome", "step")))
      throw Error("step.outcome is not done, failed or rejected");
  }
  if (type === "turn") {
    string(body, "turn", "turn");
    string(body, "to", "turn");
  }
  if (type === "usage") object(body.tokens, "usage.tokens");
  if (type === "messages" && !Array.isArray(body.messages)) throw Error("messages is not a list");
  if (type === "undo") number(body, "changed", "undo");
  if (type === "plan" || type === "parallel") checkCoordination(body);
  return {
    seq: number(e, "seq", "event"),
    at: number(e, "at", "event"),
    origin: origin as unknown as Origin,
    body: body as unknown as EventBody,
  };
}

/** The Rust session codec uses these exact discriminants. Unknown/newer records are explicit errors. */
function checkCoordination(body: Record<string, unknown>): void {
  const event = string(body, "event", String(body.type));
  const list = (o: Record<string, unknown>, key: string): unknown[] => {
    if (!Array.isArray(o[key])) throw Error(`${body.type}.${key} is not a list`);
    return o[key] as unknown[];
  };
  if (body.type === "plan") {
    if (!["proposed", "built", "ticked", "finished", "abandoned"].includes(event))
      throw Error("Unknown plan event");
    if (event === "proposed" || event === "built") {
      const plan = object(body.plan, "plan");
      string(plan, "title", "plan");
      for (const value of list(plan, "items")) {
        const item = object(value, "plan.item");
        number(item, "id", "plan.item");
        for (const key of ["name", "page", "intent"]) string(item, key, "plan.item");
      }
      if (event === "built")
        for (const value of list(body, "placeholders")) {
          const p = object(value, "placeholder");
          number(p, "item", "placeholder");
          for (const key of ["frame", "name", "page", "agent", "turn"])
            string(p, key, "placeholder");
          const bounds = object(p.bounds, "placeholder.bounds");
          for (const key of ["x", "y", "width", "height"])
            number(bounds, key, "placeholder.bounds");
        }
    } else if (event === "ticked" && list(body, "items").some((i) => !Number.isInteger(i)))
      throw Error("Plan ticks must be item ids");
  } else {
    if (!["lanes", "laneEnded", "option"].includes(event)) throw Error("Unknown parallel event");
    if (event === "lanes") {
      if (
        !["split", "side_by_side", "explore_layout", "explore_style", "spawned"].includes(
          string(body, "mode", "parallel"),
        )
      )
        throw Error("Unknown parallel mode");
      for (const value of list(body, "lanes")) {
        const l = object(value, "lane");
        for (const key of ["agent", "name", "model"]) string(l, key, "lane");
        list(l, "items");
      }
    }
    if (event === "laneEnded") {
      string(body, "agent", "laneEnded");
      if (!["done", "failed", "stopped"].includes(string(body, "end", "laneEnded")))
        throw Error("Unknown lane outcome");
    }
    if (event === "option") {
      const option = number(body, "option", "option");
      if (!Number.isInteger(option) || option < 1) throw Error("Invalid option number");
      string(body, "frame", "option");
      string(body, "agent", "option");
    }
  }
}
