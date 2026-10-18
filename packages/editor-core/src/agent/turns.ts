/**
 * Turns (DESIGN.md "Turns"): a session's events folded into what the Agent tab shows. Prose and steps interleave in
 * the order they happened (DESIGN-DECISIONS "Agent turn layout": A). A finished turn's order comes from its stored
 * messages, where each tool call sits between the texts around it; a running turn's from the engine's live stream.
 */
import { guardReasons } from "@vellum/config";
import type { AgentId, TurnId } from "@vellum/types-ts";
import { type SessionEvent, type StepBody, TERMINAL, type TurnStateName } from "./log";
import {
  describeStep,
  formatDuration,
  formatTokens,
  groupLine,
  type StepView,
  toolName,
  workingStep,
} from "./steps";

/** What the engine streamed while a turn runs, in arrival order. `step` is a recorded step that took its place. */
export type LiveItem =
  | { type: "text"; text: string }
  | { type: "working"; call: string; tool: string; startedAt: number; target?: string | null }
  | { type: "step"; seq: number };

export type TurnItem =
  | { type: "prose"; key: string; text: string; streaming: boolean }
  | { type: "step"; key: string; step: StepView }
  | { type: "group"; key: string; line: string; icon: string; steps: StepView[] };

export interface TurnFooter {
  changes: number;
  frames: number;
  durationMs: number;
  tokens: number;
}

export interface TurnView {
  /** `null` for an external agent's steps, which belong to no turn. */
  id: TurnId | null;
  key: string;
  external: boolean;
  prompt: { text: string; chips: string[] } | null;
  /** One agent, or every lane's agent for a parallel turn (a stacked header). */
  agents: AgentId[];
  model: string | null;
  state: TurnStateName;
  label: string;
  items: TurnItem[];
  footer: TurnFooter | null;
  footerLines: string[];
  latest: boolean;
  undone: { changed: number; kept: number } | null;
  canUndo: boolean;
  canRetry: boolean;
}

const LABELS: Record<TurnStateName, string> = {
  queued: "Queued",
  planning: "Planning",
  awaiting_plan: "Needs you",
  running: "Working",
  awaiting_permission: "Needs you",
  done: "Done",
  failed: "Failed",
  stopped: guardReasons.stopped,
};
export const stateLabel = (state: TurnStateName) => LABELS[state];

/** Turns with more steps than this group runs of one tool. */
const LONG_TURN = 8;
const GROUP_RUN = 3;

const changesOf = (s: StepView) =>
  s.counts.created + s.counts.changed + s.counts.removed + s.counts.variables;

function group(items: TurnItem[]): TurnItem[] {
  if (items.filter((i) => i.type === "step").length <= LONG_TURN) return items;
  const out: TurnItem[] = [];
  let run: StepView[] = [];
  const flush = () => {
    if (run.length >= GROUP_RUN) {
      const first = run[0] as StepView;
      out.push({
        type: "group",
        key: `g:${first.key}`,
        line: groupLine(run),
        icon: first.icon,
        steps: run,
      });
    } else for (const step of run) out.push({ type: "step", key: step.key, step });
    run = [];
  };
  for (const item of items) {
    const last = run[run.length - 1];
    if (
      item.type === "step" &&
      item.step.state === "done" &&
      (!last || last.tool === item.step.tool)
    )
      run.push(item.step);
    else {
      flush();
      if (item.type === "step" && item.step.state === "done") run.push(item.step);
      else out.push(item);
    }
  }
  flush();
  return out;
}

function interleave(
  events: SessionEvent[],
  steps: { seq: number; view: StepView }[],
  live: readonly LiveItem[] | undefined,
  now: number,
  running: boolean,
): TurnItem[] {
  const items: TurnItem[] = [];
  const used = new Set<number>();
  const prose = (text: string, streaming: boolean) => {
    const last = items[items.length - 1];
    if (last?.type === "prose") last.text += text;
    else items.push({ type: "prose", key: `p:${items.length}`, text, streaming });
  };
  const take = (seq: number) => {
    const s = steps.find((x) => x.seq === seq);
    if (!s || used.has(seq)) return;
    used.add(seq);
    items.push({ type: "step", key: s.view.key, step: s.view });
  };
  const messages = [...events].reverse().find((e) => e.body.type === "messages")?.body;
  if (messages?.type === "messages") {
    for (const m of messages.messages)
      if (m.role === "assistant")
        for (const part of m.parts)
          if ("text" in part) prose(part.text, false);
          else if ("toolCall" in part) {
            const tool = toolName(part.toolCall.name);
            const next = steps.find((s) => !used.has(s.seq) && s.view.tool === tool);
            if (next) take(next.seq);
          }
    for (const s of steps) take(s.seq);
    return items;
  }
  if (live?.length) {
    // Steps recorded before the panel saw the stream come first; then the stream as it arrived.
    const shown = new Set(live.flatMap((i) => (i.type === "step" ? [i.seq] : [])));
    for (const s of steps) if (!shown.has(s.seq)) take(s.seq);
    for (const item of live)
      if (item.type === "text") prose(item.text, running);
      else if (item.type === "step") take(item.seq);
      else {
        const step = workingStep(item.call, item.tool, item.startedAt, now, item.target ?? null);
        items.push({ type: "step", key: step.key, step });
      }
    for (const s of steps) take(s.seq);
    for (const item of items) if (item.type === "prose") item.streaming = running;
    return items;
  }
  for (const s of steps) take(s.seq);
  return items;
}

export function footerLines(f: TurnFooter): string[] {
  const made = [`Made ${f.changes} ${f.changes === 1 ? "change" : "changes"}`];
  if (f.frames) made.push(`${f.frames} ${f.frames === 1 ? "frame" : "frames"}`);
  const time = formatDuration(f.durationMs);
  if (!f.tokens) return [[...made, time].join(" · ")];
  return [made.join(" · "), `${time} · ${formatTokens(f.tokens)} tokens`];
}

export interface BuildOptions {
  live?: Readonly<Record<string, readonly LiveItem[]>>;
  now?: number;
  /** An external agent's session: no prompts, no Retry; its steps make one turn. */
  external?: boolean;
  /** The session's model, for a turn that has not reported usage yet. */
  model?: string | null;
}

export function buildTurns(events: readonly SessionEvent[], options: BuildOptions): TurnView[] {
  const byTurn = new Map<string, SessionEvent[]>();
  for (const e of [...events].sort((a, b) => a.seq - b.seq)) {
    const key = e.origin.turn ?? "external";
    if (!byTurn.has(key)) byTurn.set(key, []);
    byTurn.get(key)?.push(e);
  }
  const now = options.now ?? 0;
  const turns = [...byTurn].map(([key, list], index, all): TurnView => {
    const id = key === "external" ? null : (key as TurnId);
    const transitions = list.flatMap((e) => (e.body.type === "turn" ? [e.body] : []));
    const state: TurnStateName =
      transitions[transitions.length - 1]?.to ?? (options.external || !id ? "done" : "queued");
    const terminal = TERMINAL.includes(state);
    const prompt = list.find((e) => e.body.type === "prompt")?.body;
    const steps = list.flatMap((e) =>
      e.body.type === "step" ? [{ seq: e.seq, view: describeStep(e.body as StepBody, e.seq) }] : [],
    );
    const usage = list.flatMap((e) => (e.body.type === "usage" ? [e.body] : []));
    const lanes = list.find((e) => e.body.type === "parallel" && e.body.event === "lanes")?.body;
    const agents =
      lanes?.type === "parallel" && lanes.lanes?.length
        ? lanes.lanes.map((l) => l.agent)
        : [
            ...new Set(
              list.flatMap((e) => (e.origin.actor.kind === "agent" ? [e.origin.actor.agent] : [])),
            ),
          ];
    const undo = list.find((e) => e.body.type === "undo")?.body;
    const undone = undo?.type === "undo" ? { changed: undo.changed, kept: undo.kept } : null;
    const items = group(
      interleave(list, steps, id ? options.live?.[id] : undefined, now, !terminal),
    );
    const done = steps.map((s) => s.view).filter((s) => s.state === "done");
    const changes = done.reduce((n, s) => n + changesOf(s), 0);
    const frames = new Set(
      done.filter((s) => changesOf(s) > 0 && s.targets[0]).map((s) => s.targets[0]),
    ).size;
    // An Undo pressed later is not part of the turn's time.
    const timed = list.filter((e) => e.body.type !== "undo");
    const first = timed[0]?.at ?? 0;
    const last = timed[timed.length - 1]?.at ?? first;
    const footer =
      terminal || options.external
        ? {
            changes,
            frames,
            durationMs: last - first,
            tokens: usage.reduce(
              (n, u) =>
                n + u.tokens.input + u.tokens.output + u.tokens.cacheRead + u.tokens.cacheWrite,
              0,
            ),
          }
        : null;
    const latest = index === all.length - 1;
    return {
      id,
      key,
      external: !!options.external,
      prompt:
        prompt?.type === "prompt" && !options.external
          ? { text: prompt.text, chips: [...prompt.context] }
          : null,
      agents,
      model: usage[usage.length - 1]?.model ?? options.model ?? null,
      state,
      label: LABELS[state],
      items,
      footer,
      footerLines: footer ? footerLines(footer) : [],
      latest,
      undone,
      canUndo: !!footer && changes > 0 && !undone,
      canRetry: latest && terminal && !options.external && !!id,
    };
  });
  return turns;
}
