import { copy } from "@vellum/config";
import {
  type AgentController,
  type EditorSession,
  leaseLabel,
  optionLetter,
  type PlannedFrame,
  type PresenceController,
  presenceState,
} from "@vellum/editor-core";
import type { Box } from "@vellum/engine-client";
import { type CSSProperties, useEffect } from "react";
import { ActorMarker, type ActorView } from "../chrome/ActorMarker";
import { leaseColour } from "../chrome/lease-colour";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import "./presence.css";

const noStore = () => ({ dispose() {} });
export function PresenceLayer({
  presence,
  agent,
  session,
  actors,
}: {
  presence?: PresenceController;
  agent?: AgentController;
  session: EditorSession;
  actors: Readonly<Record<string, ActorView>>;
}) {
  useStore(presence?.subscribe ?? noStore);
  useStore(agent?.subscribe ?? noStore);
  useStore(session.subscribe);
  const p = presence;
  const viewport = session.state.viewport;
  const box = (b: Box): CSSProperties => ({
    left: b.x * viewport.zoom + viewport.x,
    top: b.y * viewport.zoom + viewport.y,
    width: b.width * viewport.zoom,
    height: b.height * viewport.zoom,
  });
  const actor = (id: string): ActorView =>
    actors[id] ?? {
      id,
      kind: "agent",
      name: p?.agents().find((a) => a.id === id)?.name ?? "Agent",
      hue: p?.agents().find((a) => a.id === id)?.hue,
    };
  const hue = (id: string) => leaseColour(actor(id).hue);
  useEffect(() => {
    if (!p) return;
    const timer = setInterval(() => p.tick(), 250);
    return () => clearInterval(timer);
  }, [p]);
  const plans = agent?.turns().flatMap((t) => t.orchestration.placeholders) ?? [];
  const options =
    agent?.turns().flatMap((t) =>
      t.orchestration.options.map((o) => ({
        ...o,
        turn: t.id,
        mode: t.orchestration.mode,
        model: t.orchestration.lanes.find((l) => l.agent === o.agent)?.model,
      })),
    ) ?? [];
  useEffect(() => {
    if (!p) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") p.follow(null);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [p]);
  return (
    <section className="vp-layer" aria-label="Agent presence">
      {plans
        .filter((x) => x.page === session.page)
        .map((x: PlannedFrame) => (
          <div
            key={`${x.turn}:${x.item}`}
            className="vp-placeholder"
            style={{ ...box(x.bounds), ...hue(x.agent) }}
          >
            <div className="vp-placeholder__label">
              <ActorMarker actor={actor(x.agent)} size={12} />
              <span>
                Planned · {x.name} · {actor(x.agent).name}
              </span>
            </div>
            <Icon name="square-dashed" size={24} />
            <span>Waiting to build</span>
          </div>
        ))}
      {p
        ?.leases()
        .filter((l) => l.page === session.page)
        .map((l) => (
          <div
            key={`${l.frame}:${l.agent}`}
            className="vp-lease"
            style={{ ...box(session.sceneBounds(l.frame) ?? l.bounds), ...hue(l.agent) }}
          >
            <div className="vp-lease__tab">
              <ActorMarker actor={actor(l.agent)} size={12} />
              <strong>{actor(l.agent).name}</strong>
              <span className="vp-lease__state">
                <Icon
                  name={
                    l.rejected
                      ? "triangle-alert"
                      : l.editing
                        ? "pencil"
                        : l.idle
                          ? "square"
                          : "loader-circle"
                  }
                  size={12}
                />
                {leaseLabel(l)}
              </span>
              {l.rejected ? (
                <button
                  type="button"
                  className="va-link"
                  data-error=""
                  onClick={() => p.rejected(l.frame, l.agent)}
                >
                  {l.editing ? `${l.rejected} changes rejected` : "Review"}
                </button>
              ) : null}
              <button
                type="button"
                className="va-link"
                disabled={p.state.pending}
                onClick={() => void p.action({ type: "stopFrame", agent: l.agent, frame: l.frame })}
              >
                Stop
              </button>
            </div>
          </div>
        ))}
      {p
        ?.agents()
        .filter((a) => a.cursor)
        .map((a) => (
          <div
            key={a.id}
            className="vp-cursor"
            style={{
              left: a.cursor!.x * viewport.zoom + viewport.x,
              top: a.cursor!.y * viewport.zoom + viewport.y,
              ...hue(a.id),
            }}
          >
            <Icon name="mouse-pointer-2" size={18} />
            <div className="vp-cursor__tag">
              <span>
                <ActorMarker actor={actor(a.id)} size={12} />
                <strong>{a.name}</strong> · {presenceState(a.state)}
              </span>
              {a.task ? <span>{a.task}</span> : null}
            </div>
          </div>
        ))}
      {p
        ?.highlights()
        .filter((h) => h.page === session.page)
        .map((h) => (
          <div
            key={h.node}
            className="vp-insert-highlight"
            style={{ ...box(h.bounds), ...hue(h.agent) }}
          >
            <span>Inserted by {actor(h.agent).name}</span>
          </div>
        ))}
      {options.map((o) => {
        const b = session.sceneBounds(o.frame);
        return b ? (
          <div
            key={`${o.frame}:${o.option}`}
            className="vp-alternative"
            data-highlighted={agent?.state.highlighted === o.frame ? "" : undefined}
            style={box(b)}
          >
            <span className="vp-alternative__label">
              {optionLetter(o.option)} · {actor(o.agent).name} · {o.model}
              {o.label ? ` · ${o.label}` : ""}
            </span>
            {o.turn ? (
              <button
                type="button"
                className="va-button vp-alternative__keep"
                aria-label={`Keep ${optionLetter(o.option)} on canvas`}
                onClick={() => agent?.chooseOption(o.turn!, o.option)}
              >
                Keep {optionLetter(o.option)}
              </button>
            ) : null}
          </div>
        ) : null;
      })}
      {p?.state.snapshot.connected && p.state.snapshot.paused ? (
        <div className="vp-paused" role="status">
          <Icon name="triangle-alert" size={16} />
          <span>{copy.pausedBanner}</span>
          <button
            type="button"
            className="va-link"
            disabled={p.state.pending}
            onClick={() => void p.action({ type: "resumeAll" })}
          >
            Resume
          </button>
        </div>
      ) : null}
      {p?.state.following ? (
        <button type="button" className="vp-follow" onClick={() => p.follow(null)}>
          <Icon name="focus" size={16} />
          {`Following ${actor(p.state.following).name} · Esc to stop`}
        </button>
      ) : null}
      {p?.state.error ? (
        <p className="vp-error" role="alert">
          {p.state.error}
        </p>
      ) : null}
    </section>
  );
}
