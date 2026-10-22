import {
  type AgentController,
  type AgentPlan,
  editPlan,
  type OrchestrationView,
  optionLetter,
  type PlanBuild,
  type TurnView,
  validatePlan,
} from "@vellum/editor-core";
import { useState } from "react";
import { useLocale } from "../accessibility/locale";
import { ActorMarker, type ActorView } from "../chrome/ActorMarker";
import { Icon } from "../icons/Icon";
import { TurnItems } from "./Steps";
import "./orchestration.css";
export function PlanCard({
  view,
  turn,
  agent,
  pageName,
  models = [],
}: {
  view: OrchestrationView;
  turn: string;
  agent: AgentController;
  pageName: (id: string) => string;
  models?: string[];
}) {
  const { t: translateUi } = useLocale();
  const [draft, setDraft] = useState<AgentPlan>(() => view.plan!);
  const [parallel, setParallel] = useState<PlanBuild>({ count: 4, mode: "split" });
  const [editing, setEditing] = useState<number | null>(null);
  const [request, setRequest] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const proposed = view.status === "proposed";
  const plan = proposed ? draft : view.plan!;
  const invalid = validatePlan(plan);
  const groups = [...new Set(plan.items.map((i) => i.page))];
  const submit = async () => {
    setPending(true);
    await agent.buildPlan(turn, plan, parallel);
    setPending(!agent.state.error);
  };
  return (
    <section className="va-plan" aria-label="Plan" data-status={view.status}>
      <h3>{`Plan · ${plan.title}`}</h3>
      <p className="va-plan__state">
        <Icon name={proposed ? "triangle-alert" : "check"} size={12} />
        {proposed
          ? "Needs you · review before building"
          : `${view.done.length} of ${plan.items.length} screens done`}
      </p>
      {groups.map((page) => (
        <div className="va-plan__group" key={page}>
          <h4>
            {pageName(page)} · {plan.items.filter((i) => i.page === page).length}
          </h4>
          {plan.items
            .filter((i) => i.page === page)
            .map((item) => (
              <div key={item.id} className="va-plan__item">
                <div className="va-plan__row">
                  <span aria-hidden="true">
                    {view.done.includes(item.id) ? (
                      <Icon name="check" size={12} />
                    ) : (
                      <Icon name="grip-vertical" size={12} />
                    )}
                  </span>
                  <span title={item.intent}>{item.name}</span>
                  {proposed ? (
                    <button
                      type="button"
                      className="vc-iconbutton"
                      aria-label={`Edit ${item.name}`}
                      onClick={() => setEditing(editing === item.id ? null : item.id)}
                    >
                      <Icon name="pencil" size={12} />
                    </button>
                  ) : null}
                </div>
                {editing === item.id && proposed ? (
                  <fieldset disabled={pending} className="va-plan__edit">
                    <label>
                      Screen name
                      <input
                        value={item.name}
                        onChange={(e) =>
                          setDraft(
                            editPlan(draft, {
                              type: "update",
                              id: item.id,
                              patch: { name: e.target.value },
                            }),
                          )
                        }
                      />
                    </label>
                    <label>
                      Page
                      <input
                        value={item.page}
                        onChange={(e) =>
                          setDraft(
                            editPlan(draft, {
                              type: "update",
                              id: item.id,
                              patch: { page: e.target.value },
                            }),
                          )
                        }
                      />
                    </label>
                    <label>
                      Screen intent
                      <textarea
                        value={item.intent}
                        onChange={(e) =>
                          setDraft(
                            editPlan(draft, {
                              type: "update",
                              id: item.id,
                              patch: { intent: e.target.value },
                            }),
                          )
                        }
                      />
                    </label>
                    <div className="va-actions">
                      <button
                        type="button"
                        className="va-button"
                        disabled={plan.items.indexOf(item) === 0}
                        onClick={() =>
                          setDraft(
                            editPlan(draft, {
                              type: "move",
                              id: item.id,
                              index: plan.items.indexOf(item) - 1,
                            }),
                          )
                        }
                      >
                        Move up
                      </button>
                      <button
                        type="button"
                        className="va-button"
                        disabled={plan.items.indexOf(item) === plan.items.length - 1}
                        onClick={() =>
                          setDraft(
                            editPlan(draft, {
                              type: "move",
                              id: item.id,
                              index: plan.items.indexOf(item) + 1,
                            }),
                          )
                        }
                      >
                        Move down
                      </button>
                      <button
                        type="button"
                        className="va-link"
                        onClick={() => {
                          setDraft(editPlan(draft, { type: "remove", id: item.id }));
                          setEditing(null);
                        }}
                      >
                        Remove screen
                      </button>
                      <button type="button" className="va-button" onClick={() => setEditing(null)}>
                        Done editing
                      </button>
                    </div>
                  </fieldset>
                ) : null}
              </div>
            ))}
        </div>
      ))}
      {proposed ? (
        <>
          <fieldset disabled={pending} className="va-plan__parallel">
            <span>{translateUi("Parallel")}</span>
            <button
              type="button"
              className="va-button"
              aria-label="Fewer plan agents"
              disabled={parallel.count <= 1}
              onClick={() => setParallel({ ...parallel, count: parallel.count - 1 })}
            >
              −
            </button>
            <output>{parallel.count}</output>
            <button
              type="button"
              className="va-button"
              aria-label="More plan agents"
              disabled={parallel.count >= 6}
              onClick={() => setParallel({ ...parallel, count: parallel.count + 1 })}
            >
              +
            </button>
            <select
              aria-label="Plan mode"
              value={parallel.mode}
              onChange={(e) =>
                setParallel({ ...parallel, mode: e.target.value as PlanBuild["mode"] })
              }
            >
              <option value="split">Split</option>
              <option value="side_by_side">Side by side</option>
            </select>
          </fieldset>
          <p className="va-plan__explanation">
            {parallel.mode === "split"
              ? `${parallel.count} agents, about ${Math.ceil(plan.items.length / parallel.count)} screens each`
              : `${parallel.count} agents, each builds the whole request`}
          </p>
          {parallel.mode === "side_by_side"
            ? Array.from({ length: parallel.count }, (_, i) => (
                <label key={i} className="va-plan__model">
                  Lane {optionLetter(i + 1)} model
                  <select
                    aria-label={`Lane ${optionLetter(i + 1)} model`}
                    value={parallel.models?.[i] ?? models[0] ?? ""}
                    onChange={(e) => {
                      const next = Array.from(
                        { length: parallel.count },
                        (_, n) => parallel.models?.[n] ?? models[0] ?? "",
                      );
                      next[i] = e.target.value;
                      setParallel({ ...parallel, models: next });
                    }}
                  >
                    <option value="">Session default</option>
                    {models.map((m) => (
                      <option key={m}>{m}</option>
                    ))}
                  </select>
                </label>
              ))
            : null}
          {invalid ? <p role="alert">{invalid}</p> : null}
          {request !== null ? (
            <label className="va-plan__request">
              Edit request
              <textarea value={request} onChange={(e) => setRequest(e.target.value)} />
              <button
                type="button"
                className="va-button"
                disabled={pending || !request.trim()}
                onClick={async () => {
                  setPending(true);
                  await agent.editRequest(turn, request);
                  setPending(false);
                }}
              >
                Update request
              </button>
            </label>
          ) : null}
          <div className="va-actions va-plan__actions">
            <button
              type="button"
              className="va-button"
              data-primary=""
              disabled={pending || !!invalid}
              onClick={() => void submit()}
            >
              Build
            </button>
            <button
              type="button"
              className="va-button"
              disabled={pending}
              onClick={() =>
                setRequest(agent.turns().find((t) => t.id === turn)?.prompt?.text ?? "")
              }
            >
              Edit request
            </button>
          </div>
        </>
      ) : null}
    </section>
  );
}
export function ParallelTurn({
  turn,
  agent,
  actor,
}: {
  turn: TurnView;
  agent: AgentController;
  actor: (id: string) => ActorView;
}) {
  const v = turn.orchestration;
  const selected = agent.state.selectedOptions[turn.key] ?? [];
  const [kept, setKept] = useState<number[] | null>(null);
  const [pending, setPending] = useState(false);
  return (
    <div className="va-parallel">
      {v.plan ? (
        <>
          <p className="va-parallel__progress">
            {v.done.length} of {v.plan.items.length} screens done
          </p>
          <progress aria-label="Screens complete" value={v.done.length} max={v.plan.items.length} />
          <p className="va-parallel__groups">
            {[...new Set(v.plan.items.map((i) => i.page))]
              .map(
                (page) =>
                  `${agent.pageName(page)} ${v.plan!.items.filter((i) => i.page === page && v.done.includes(i.id)).length}/${v.plan!.items.filter((i) => i.page === page).length}`,
              )
              .join(" · ")}
          </p>
        </>
      ) : null}
      {v.plan ? (
        <details className="va-live-plan">
          <summary>
            Plan items · {v.done.length}/{v.plan.items.length}
          </summary>
          {v.plan.items.map((i) => (
            <p key={i.id}>
              <Icon name={v.done.includes(i.id) ? "check" : "square"} size={12} /> {i.name} ·{" "}
              {agent.pageName(i.page)}
            </p>
          ))}
        </details>
      ) : null}
      {v.lanes.map((l) => {
        const key = `lane:${turn.key}:${l.agent}`;
        const expanded = agent.expanded(key);
        return (
          <section className="va-lane" key={l.agent} data-agent={l.agent}>
            <button
              type="button"
              className="va-lane__header"
              aria-expanded={expanded}
              onClick={() => agent.toggle(key)}
            >
              <ActorMarker actor={actor(l.agent)} size={16} />
              <span>
                {l.name}
                {l.task
                  ? ` · ${l.task}`
                  : v.plan && l.planItems.length
                    ? ` · ${agent.pageName(v.plan.items.find((i) => l.planItems.includes(i.id))?.page ?? "")}`
                    : ""}
              </span>
              <small>
                {l.end ?? `${l.planItems.filter((i) => v.done.includes(i)).length} done`}
              </small>
              <Icon name="chevron-down" size={12} />
            </button>
            {expanded ? (
              <>
                <p className="va-lane__model vc-mono">{l.model}</p>
                <TurnItems items={l.items} agent={agent} turn={turn.id} />
                {!l.items.length ? <p className="va-note">{l.end ?? "Working"}</p> : null}
              </>
            ) : (
              <p className="va-lane__summary">
                {l.items.findLast((i) => i.type === "step")?.type === "step"
                  ? l.items.filter((i) => i.type === "step").at(-1)?.step.line
                  : (l.end ?? "Working")}
              </p>
            )}
          </section>
        );
      })}
      {kept ? <p role="status">Kept {kept.map(optionLetter).join(", ")}</p> : null}
      {v.options.length ? (
        <section
          className="va-alternatives"
          aria-label={
            v.mode?.startsWith("explore") ? "Explore alternatives" : "Side by side alternatives"
          }
        >
          {v.options.map((o) => (
            <div key={o.option} className="va-option">
              <button
                type="button"
                className="va-link"
                aria-label={`Preview ${optionLetter(o.option)}`}
                onMouseEnter={() => agent.highlight(o.frame)}
                onMouseLeave={() => agent.highlight(null)}
                onFocus={() => agent.highlight(o.frame)}
                onBlur={() => agent.highlight(null)}
                onClick={() => agent.zoomTo(o.frame)}
              >
                <span className="va-option__thumbnail" aria-hidden="true">
                  {agent.thumbnail(o.frame) ? (
                    <img src={agent.thumbnail(o.frame)!} alt="" />
                  ) : (
                    <Icon name="image" size={16} />
                  )}
                </span>
                <span>
                  {optionLetter(o.option)} ·{" "}
                  {o.label ??
                    v.lanes.find((l) => l.agent === o.agent)?.model ??
                    actor(o.agent).name}
                </span>
              </button>
              <button
                type="button"
                className="va-button"
                aria-pressed={selected.includes(o.option)}
                onClick={() => agent.chooseOption(turn.id!, o.option)}
              >
                Keep {optionLetter(o.option)}
              </button>
            </div>
          ))}
          <button
            type="button"
            className="va-button"
            data-primary=""
            disabled={!selected.length || pending}
            onClick={async () => {
              setPending(true);
              await agent.keepOptions(turn.id!, selected);
              setKept(agent.state.error ? null : selected);
              setPending(false);
            }}
          >
            {selected.length
              ? `Keep ${selected.map(optionLetter).join(", ")}, discard others`
              : "Choose an option to keep"}
          </button>
        </section>
      ) : null}
    </div>
  );
}
