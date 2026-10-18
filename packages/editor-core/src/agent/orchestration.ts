/** Read models for the existing Rust session codec (plans and parallel). Never document content. */

import type { Box } from "@vellum/engine-client";
import type { AgentId, NodeId, PageId, TurnId } from "@vellum/types-ts";
import type { ParallelMode } from "./feed";
import type { SessionEvent } from "./log";
import type { TurnItem } from "./turns";
export interface PlanItem {
  id: number;
  name: string;
  page: PageId;
  intent: string;
  width?: number;
  height?: number;
  frame?: NodeId;
}
export interface AgentPlan {
  title: string;
  items: PlanItem[];
  changes?: number;
}
export interface PlannedFrame {
  item: number;
  frame: NodeId;
  name: string;
  page: PageId;
  bounds: Box;
  agent: AgentId;
  turn: TurnId;
}
export type PlanEdit =
  | { type: "update"; id: number; patch: Partial<Omit<PlanItem, "id">> }
  | { type: "move"; id: number; index: number }
  | { type: "remove"; id: number };
export interface PlanBuild {
  count: number;
  mode: ParallelMode /** Per-lane model in Side by side; the daemon validates availability. */;
  models?: string[];
}
export interface AgentLane {
  agent: AgentId;
  name: string;
  model: string;
  task?: string;
  planItems: number[];
  end: "done" | "failed" | "stopped" | null;
  items: TurnItem[];
}
export interface Alternative {
  option: number;
  agent: AgentId;
  frame: NodeId;
  label?: string;
  thumbnail?: string;
}
export interface OrchestrationView {
  plan: AgentPlan | null;
  revision: number;
  status: "proposed" | "building" | "finished" | "abandoned" | null;
  done: number[];
  placeholders: PlannedFrame[];
  mode: string | null;
  lanes: AgentLane[];
  options: Alternative[];
}
export function validatePlan(plan: AgentPlan): string | null {
  if (!plan.title.trim()) return "Give the plan a title";
  if (!plan.items.length) return "Add at least one screen";
  if (new Set(plan.items.map((i) => i.id)).size !== plan.items.length)
    return "Plan item ids must be unique";
  for (const i of plan.items) {
    if (!i.name.trim() || !i.intent.trim() || !i.page)
      return "Every screen needs a name, page and intent";
    if (
      !i.frame &&
      (!(typeof i.width === "number" && Number.isFinite(i.width) && i.width > 0) ||
        !(typeof i.height === "number" && Number.isFinite(i.height) && i.height > 0))
    )
      return "Every new screen needs a positive width and height";
  }
  return null;
}
export function editPlan(plan: AgentPlan, edit: PlanEdit): AgentPlan {
  const items = plan.items.map((i) => ({ ...i }));
  const at = items.findIndex((i) => i.id === edit.id);
  if (at < 0) return plan;
  if (edit.type === "update") items[at] = { ...items[at]!, ...edit.patch };
  else {
    const [item] = items.splice(at, 1);
    if (edit.type === "move" && item)
      items.splice(Math.max(0, Math.min(items.length, edit.index)), 0, item);
  }
  return { ...plan, items };
}
export function reduceOrchestration(events: readonly SessionEvent[]): OrchestrationView {
  const v: OrchestrationView = {
    plan: null,
    revision: 0,
    status: null,
    done: [],
    placeholders: [],
    mode: null,
    lanes: [],
    options: [],
  };
  for (const e of [...events].sort((a, b) => a.seq - b.seq)) {
    const b = e.body;
    if (b.type === "plan") {
      if (b.event === "proposed" || b.event === "built") {
        v.plan = b.plan as AgentPlan;
        v.revision = e.seq;
        v.status = b.event === "built" ? "building" : "proposed";
        v.done = [];
        v.placeholders = b.event === "built" ? (b.placeholders as PlannedFrame[]) : [];
      } else if (b.event === "ticked")
        v.done = [...new Set([...v.done, ...(b.items as number[])])].filter((id) =>
          v.plan?.items.some((i) => i.id === id),
        );
      else if (b.event === "finished" || b.event === "abandoned") {
        v.status = b.event;
        v.placeholders = [];
      }
    } else if (b.type === "parallel") {
      if (b.event === "lanes") {
        v.mode = b.mode as string;
        v.lanes = (b.lanes ?? []).map((l) => ({
          agent: l.agent,
          name: typeof l.name === "string" ? l.name : "Agent",
          model: typeof l.model === "string" ? l.model : "Model unavailable",
          planItems: Array.isArray(l.items) ? (l.items as number[]) : [],
          ...(l.task ? { task: l.task as string } : {}),
          end: null,
          items: [],
        }));
      } else if (b.event === "laneEnded") {
        const l = v.lanes.find((l) => l.agent === b.agent);
        if (l) l.end = b.end as AgentLane["end"];
      } else if (b.event === "option") {
        const o = b as unknown as Alternative;
        v.options = [
          ...v.options.filter((x) => x.option !== o.option),
          {
            option: o.option,
            agent: o.agent,
            frame: o.frame,
            ...(o.label ? { label: o.label } : {}),
          },
        ].sort((a, b) => a.option - b.option);
      }
    }
    if (b.type === "turn" && ["done", "failed", "stopped"].includes(b.to)) v.placeholders = [];
  }
  v.placeholders = v.placeholders.filter((p) => !v.done.includes(p.item));
  return v;
}
export const optionLetter = (n: number) => String.fromCharCode(64 + n);
