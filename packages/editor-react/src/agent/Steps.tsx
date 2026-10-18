/** Step Timeline rows (DESIGN.md "Step Timeline"): collapsed by default, details on expand. */
import { copy } from "@vellum/config";
import type { AgentController, StepView, TurnItem } from "@vellum/editor-core";
import { useState } from "react";
import { Icon } from "../icons/Icon";
import { DecidedPermission } from "./PermissionCard";

const STATE_ICON = {
  working: "loader-circle",
  done: "check",
  warning: "triangle-alert",
  failed: "circle-alert",
} as const;
/** Said in words beside the symbol for assistive tech: state is never colour alone. */
const STATE_WORDS = {
  working: "working",
  done: "done",
  warning: "refused",
  failed: "failed",
} as const;

export function CopyBlock({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="va-block">
      <pre className="va-block__text">{text}</pre>
      <button
        type="button"
        className="va-link"
        aria-label={`Copy ${label}`}
        onClick={() => {
          void navigator.clipboard?.writeText(text).then(() => setCopied(true));
        }}
      >
        {copied ? "Copied" : "Copy"}
      </button>
    </div>
  );
}

function Thumbnail({ step, agent }: { step: StepView; agent: AgentController }) {
  const t = step.thumbnail;
  if (!t) return null;
  const url = agent.image(t.image);
  const name = t.name ?? "Frame";
  const picture = url ? (
    <img className="va-thumb__image" src={url} alt="" width={40} height={48} />
  ) : (
    <span className="va-thumb__image" data-missing="" aria-hidden />
  );
  const label = t.node ? `${name} · zoom to frame` : name;
  return t.node ? (
    <button
      type="button"
      className="va-thumb"
      onClick={() => agent.zoomTo(t.node as string)}
      aria-label={label}
    >
      {picture}
      <span className="va-thumb__label">{label}</span>
    </button>
  ) : (
    <div className="va-thumb">
      {picture}
      <span className="va-thumb__label">{label}</span>
    </div>
  );
}

export function StepRow({
  step,
  agent,
  turn,
}: {
  step: StepView;
  agent: AgentController;
  turn: string | null;
}) {
  const open = agent.expanded(step.key);
  const details = `va-step-${step.key.replace(/\W/g, "-")}`;
  // Designed steps always show their frame; a checked screenshot shows its picture when expanded.
  const thumbnail = step.kind === "designed" || (step.kind === "screenshot" && open);
  return (
    <>
      <button
        type="button"
        className="va-step"
        data-state={step.state}
        aria-expanded={open}
        aria-controls={details}
        onClick={() => agent.toggle(step.key)}
      >
        <Icon name={step.icon} className="va-step__icon" />
        <span className="va-step__line">{step.line}</span>
        <span className="va-step__state">
          <Icon name={STATE_ICON[step.state]} size={14} />
          <span className="va-sr">{STATE_WORDS[step.state]}</span>
        </span>
        <span className="va-step__time vc-mono">{step.duration}</span>
      </button>
      {thumbnail ? <Thumbnail step={step} agent={agent} /> : null}
      {step.stuck ? (
        <section className="va-stuck" aria-label={copy.agentStuck}>
          <h4 role="status">{copy.agentStuck}</h4>
          <p>You can stop this step or retry it. The screens already built are kept.</p>
          <div className="va-actions">
            <button
              type="button"
              className="va-button"
              data-primary=""
              onClick={() => void agent.stop()}
            >
              Stop
            </button>
            <button
              type="button"
              className="va-button"
              disabled={!turn}
              onClick={() => turn && void agent.retry(turn)}
            >
              Retry step
            </button>
          </div>
        </section>
      ) : null}
      {step.verdict && (open || step.kind === "screenshot") ? (
        <p className="va-note va-note--text">{step.verdict}</p>
      ) : null}
      {step.problems ? <p className="va-note va-note--text">{step.problems}</p> : null}
      {step.state === "failed" || step.state === "warning" ? (
        <>
          {step.error ? <p className="va-note va-note--text">{step.error}</p> : null}
          {step.state === "failed" && turn && step.seq !== null ? (
            <button
              type="button"
              className="va-button"
              onClick={() => void agent.retryStep(turn, step.seq as number)}
            >
              Retry step
            </button>
          ) : null}
        </>
      ) : null}
      {open ? (
        <div className="va-details" id={details}>
          {step.targets.length ? (
            <p className="va-details__targets vc-mono">{step.targets.join("\n")}</p>
          ) : null}
          {step.counts.created +
          step.counts.changed +
          step.counts.removed +
          step.counts.variables ? (
            <p className="va-details__counts">
              {[
                step.counts.created ? `${step.counts.created} created` : null,
                step.counts.changed ? `${step.counts.changed} changed` : null,
                step.counts.removed ? `${step.counts.removed} removed` : null,
                step.counts.variables ? `${step.counts.variables} variables` : null,
              ]
                .filter(Boolean)
                .join(" · ")}
            </p>
          ) : null}
          {step.script ? <CopyBlock text={step.script} label="script" /> : null}
          {step.output ? <CopyBlock text={step.output} label="output" /> : null}
        </div>
      ) : (
        <span id={details} hidden />
      )}
    </>
  );
}

function GroupRow({
  item,
  agent,
  turn,
}: {
  item: Extract<TurnItem, { type: "group" }>;
  agent: AgentController;
  turn: string | null;
}) {
  const open = agent.expanded(item.key);
  return (
    <>
      <button
        type="button"
        className="va-step"
        data-state="done"
        aria-expanded={open}
        onClick={() => agent.toggle(item.key)}
      >
        <Icon name={item.icon} className="va-step__icon" />
        <span className="va-step__line">{item.line}</span>
        <span className="va-step__state">
          <Icon name={open ? "chevron-up" : "chevron-down"} size={14} />
        </span>
      </button>
      {open
        ? item.steps.map((s) => <StepRow key={s.key} step={s} agent={agent} turn={turn} />)
        : null}
    </>
  );
}

/** Prose and steps in the order they happened; consecutive steps sit together as one timeline. */
export function TurnItems({
  items,
  agent,
  turn,
}: {
  items: readonly TurnItem[];
  agent: AgentController;
  turn: string | null;
}) {
  const runs: TurnItem[][] = [];
  for (const item of items) {
    const last = runs[runs.length - 1];
    const loose = item.type === "prose" || item.type === "permission";
    if (!loose && last && last[0]?.type !== "prose" && last[0]?.type !== "permission")
      last.push(item);
    else runs.push([item]);
  }
  return (
    <>
      {runs.map((run) => {
        const first = run[0] as TurnItem;
        if (first.type === "prose")
          return (
            <p key={first.key} className="va-prose" aria-busy={first.streaming || undefined}>
              {first.text}
            </p>
          );
        if (first.type === "permission")
          return <DecidedPermission key={first.key} card={first.card} />;
        return (
          <div key={first.key} className="va-steps">
            {run.map((item) =>
              item.type === "step" ? (
                <StepRow key={item.key} step={item.step} agent={agent} turn={turn} />
              ) : item.type === "group" ? (
                <GroupRow key={item.key} item={item} agent={agent} turn={turn} />
              ) : null,
            )}
          </div>
        );
      })}
    </>
  );
}
