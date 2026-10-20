import type {
  CollaborationController,
  EditorSession,
  PresenceController,
} from "@vellum/editor-core";
import type { Participant, Role } from "@vellum/types-ts";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { ActorMarker } from "../chrome/ActorMarker";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import "./collaboration.css";
import { CommentsPanel } from "./CommentsPanel";

export { CommentsPanel } from "./CommentsPanel";

function ConnectionNotice({
  modal,
  ready,
  label,
  children,
}: {
  modal: boolean;
  ready: boolean;
  label: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (modal && ref.current && !ref.current.open) ref.current.showModal();
  }, [modal]);
  return modal ? (
    <dialog className="vm-access" ref={ref} aria-label={label} onCancel={(e) => e.preventDefault()}>
      {children}
    </dialog>
  ) : (
    <section className="vm-sync" data-ready={ready || undefined} role="status" aria-label={label}>
      {children}
    </section>
  );
}

function Mark({ participant: p, filled = false }: { participant: Participant; filled?: boolean }) {
  const name = p.state?.name ?? (p.actor.kind === "agent" ? "Agent" : "Person");
  return p.actor.kind === "agent" ? (
    <ActorMarker
      actor={{
        id: p.connection,
        kind: "agent",
        name,
        ...(p.state?.hue ? { hue: p.state.hue } : {}),
      }}
      size={22}
    />
  ) : (
    <span
      className="vm-person"
      data-outline={!filled || undefined}
      role="img"
      aria-label={`${name} · Person`}
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
}
export function CollaborationToolbar({
  controller: c,
  presence,
  onShare,
  onSettings,
}: {
  controller: CollaborationController;
  presence?: PresenceController;
  onShare?: () => void;
  onSettings?: () => void;
}) {
  useStore(c.subscribe);
  useStore(presence?.subscribe ?? (() => ({ dispose() {} })));
  const ref = useRef<HTMLDivElement>(null);
  const participants = [...c.participants(), ...c.lastKnown()];
  const active = c.participants();
  useEffect(() => {
    if (!c.state.menu) return;
    const dismiss = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) c.menu(false);
    };
    document.addEventListener("pointerdown", dismiss);
    return () => document.removeEventListener("pointerdown", dismiss);
  }, [c, c.state.menu]);
  return (
    <div
      className="vm-toolbar vc-bar"
      ref={ref}
      role="toolbar"
      aria-label="Presence and sharing"
      onKeyDown={(e) => {
        if (e.key === "Escape" && c.state.menu) {
          e.stopPropagation();
          c.menu(false);
          ref.current?.querySelector<HTMLButtonElement>("button")?.focus();
        }
      }}
    >
      <button
        type="button"
        aria-label="In this document"
        aria-expanded={c.state.menu}
        onClick={() => c.menu(!c.state.menu)}
      >
        <span className="vm-stack">
          {participants.slice(0, 6).map((p) => (
            <span key={p.connection} data-stale={!c.isLive(p) || undefined}>
              <Mark
                participant={p}
                filled={JSON.stringify(c.state.grant?.actor) === JSON.stringify(p.actor)}
              />
            </span>
          ))}
        </span>
        <strong>{c.connection === "ready" ? active.length : "—"}</strong>
        <Icon name="chevron-down" size={12} />
      </button>
      <button
        className="vm-comments-entry"
        type="button"
        aria-pressed={c.state.panel}
        disabled={c.connection !== "ready"}
        onClick={() => (c.state.panel ? c.close() : c.open())}
      >
        <Icon name="message-square" size={15} /> Comments{" "}
        <strong>{c.threads().filter(([, t]) => !t.resolved).length}</strong>
      </button>
      {onShare && (
        <button type="button" className="vm-share-entry" onClick={onShare}>
          Share
        </button>
      )}
      {onSettings && (
        <button type="button" onClick={onSettings} aria-label="Editor settings">
          <Icon name="settings" size={16} />
        </button>
      )}
      {c.state.menu && (
        <section className="vm-participants" aria-label="Participants">
          <header>
            <strong>In this document</strong>
            <span>
              {active.filter((p) => p.actor.kind === "person").length} people ·{" "}
              {active.filter((p) => p.actor.kind === "agent").length} agents
            </span>
          </header>
          {(["person", "agent"] as const).map((kind) => (
            <section key={kind} aria-label={kind === "person" ? "People" : "Agents"}>
              <h3>{kind === "person" ? "People" : "Agents"}</h3>
              {participants
                .filter((p) => p.actor.kind === kind)
                .map((p) => (
                  <div
                    className="vm-participant"
                    key={p.connection}
                    data-stale={!c.isLive(p) || undefined}
                  >
                    <Mark participant={p} />
                    <span>
                      <strong>{p.state?.name ?? (kind === "agent" ? "Agent" : "Person")}</strong>
                      <small>
                        {c.isLive(p)
                          ? `${kind === "agent" ? "Agent" : "Person"} · ${p.state?.activity || "Viewing"}`
                          : "Disconnected · Not live"}
                      </small>
                    </span>
                    <button
                      type="button"
                      disabled={!c.isLive(p) || !p.state?.viewport}
                      onClick={() => {
                        c.follow(p.connection);
                        c.menu(false);
                      }}
                    >
                      Follow
                    </button>
                    {p.actor.kind === "agent" &&
                      presence
                        ?.agents()
                        .some((a) => p.actor.kind === "agent" && a.id === p.actor.agent) && (
                        <>
                          <button
                            type="button"
                            onClick={() => {
                              if (p.actor.kind === "agent") presence.openSession(p.actor.agent);
                            }}
                          >
                            Open session
                          </button>
                          <button
                            type="button"
                            disabled={presence.state.pending}
                            onClick={() => {
                              if (p.actor.kind === "agent")
                                void presence.action({ type: "stopAgent", agent: p.actor.agent });
                            }}
                          >
                            Stop this agent
                          </button>
                        </>
                      )}
                  </div>
                ))}
            </section>
          ))}
          {presence && (
            <button
              type="button"
              disabled={!presence.state.snapshot.connected || presence.state.pending}
              onClick={() => void presence.action({ type: "pauseAll" })}
            >
              Pause all agents
            </button>
          )}
        </section>
      )}
    </div>
  );
}
export function CollaborationLayer({
  controller: c,
  session,
}: {
  controller: CollaborationController;
  session: EditorSession;
}) {
  useStore(c.subscribe);
  useStore(session.subscribe);
  useEffect(() => {
    if (!c.state.following) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented) c.follow(null);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [c, c.state.following]);
  useEffect(() => {
    const timer = setInterval(() => {
      c.tick();
      c.publish();
    }, 5000);
    return () => clearInterval(timer);
  }, [c]);
  const v = session.state.viewport;
  useEffect(() => {
    c.publishView({
      page: session.page,
      cursor: null,
      viewport: session.state.viewport,
      selection: [...session.state.selection],
    });
  }, [c, session, session.state.viewport, session.state.selection, session.page]);
  const remote = c.participants();
  const [dismissed, setDismissed] = useState("");
  const status = c.connection;
  useEffect(() => {
    if (status === "ready") {
      c.publish();
      void c.loadSharing();
    }
  }, [c, status]);
  const title =
    status === "connecting"
      ? "Connecting to live collaboration"
      : status === "ready"
        ? "Live collaboration on"
        : status === "forbidden"
          ? "Your access was revoked"
          : status === "incompatible"
            ? "Incompatible sync version"
            : status === "needs-review"
              ? "Room was reset"
              : "Connection lost";
  const blocked = ["forbidden", "incompatible", "needs-review"].includes(status);
  return (
    <section className="vm-layer" aria-label="Live collaboration">
      {c.state.placing && (
        <p className="vm-role" role="status">
          Click the canvas to place a comment · Esc to cancel
        </p>
      )}
      {remote
        .filter((p) => p.state?.cursor && (!p.state.page || p.state.page === session.page))
        .map((p) => (
          <div
            className="vm-cursor"
            data-actor={p.actor.kind}
            key={p.connection}
            style={{
              left: p.state!.cursor!.x * v.zoom + v.x,
              top: p.state!.cursor!.y * v.zoom + v.y,
            }}
          >
            <Icon name="mouse-pointer-2" size={18} />
            <div>
              <Mark participant={p} />
              <strong>{p.state!.name}</strong>
              <small>
                {p.actor.kind === "agent" ? "Agent" : "Person"} · {p.state!.activity}
              </small>
            </div>
          </div>
        ))}
      {c.threads().map(([id, t], i) =>
        !t.resolved && (!t.page || t.page === session.page) ? (
          <button
            type="button"
            className="vm-pin"
            aria-label={`Comment ${i + 1}`}
            aria-pressed={c.state.selected === id}
            key={id}
            style={{ left: t.point.x * v.zoom + v.x, top: t.point.y * v.zoom + v.y }}
            onClick={() => c.select(id)}
          >
            {i + 1}
          </button>
        ) : null,
      )}
      {c.connection === "ready" &&
        c
          .lastKnown()
          .filter((p) => p.actor.kind === "person" && p.state?.cursor)
          .map((p) => (
            <span
              className="vm-last-seen"
              key={p.connection}
              style={{
                left: p.state!.cursor!.x * v.zoom + v.x,
                top: p.state!.cursor!.y * v.zoom + v.y,
              }}
            >
              {p.state!.name} · Not live
            </span>
          ))}
      {c.state.following && (
        <div className="vm-follow">
          Following {remote.find((p) => p.connection === c.state.following)?.state?.name} · Esc to
          stop{" "}
          <button type="button" onClick={() => c.follow(null)}>
            Return to your view
          </button>
        </div>
      )}
      {c.state.returned && (
        <p className="vm-return" role="status">
          Returned to your own view
        </p>
      )}
      <output className="vm-sync-status">
        {c.connection === "ready"
          ? `✓ Sync connected · ${remote.length} here`
          : `△ ${c.connection === "connecting" ? "Sync reconnecting" : "Sync unavailable"}`}
      </output>
      {c.state.active &&
        dismissed !== status &&
        (status !== "ready" || c.state.grant?.role === "editor") && (
          <ConnectionNotice modal={blocked} ready={status === "ready"} label={title}>
            {!blocked && (
              <span className="vm-sync-icon">
                <Icon name={status === "ready" ? "check" : "refresh-cw"} size={16} />
              </span>
            )}
            <div className="vm-sync-copy">
              <strong>{title}</strong>
              <p>
                {status === "ready"
                  ? `${c.state.sharing?.server ?? "This document"} · ${remote.length} here`
                  : status === "forbidden"
                    ? `Your local file is unchanged. Unsent edits are ${c.persistent ? "saved locally" : "held in this session"}.`
                    : status === "needs-review"
                      ? `${c.pending} local edits are not on the server. Review them before rebasing.`
                      : status === "incompatible"
                        ? "Update Vellum before retrying. Nothing is discarded automatically."
                        : `${c.pending} unsent edits ${c.persistent ? "saved locally" : "held in this session"}. Remote presence is unavailable.`}
              </p>
            </div>
            <div className="vm-actions">
              {blocked ? (
                <>
                  <button type="button" onClick={() => void c.copyUnsent()}>
                    Copy unsent edits
                  </button>
                  <button type="button" onClick={() => void c.workOffline()}>
                    Keep local copy
                  </button>
                  {status === "needs-review" && (
                    <button
                      type="button"
                      disabled={!c.canRebase}
                      onClick={() => void c.reviewRebase()}
                    >
                      Review and rebase
                    </button>
                  )}
                  {status === "incompatible" && (
                    <button type="button" onClick={() => c.update()}>
                      Update Vellum
                    </button>
                  )}
                </>
              ) : (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      if (status === "ready") c.menu(true);
                      else void c.workOffline();
                    }}
                  >
                    {status === "ready" ? "Show participants" : "Work offline"}
                  </button>
                  <button
                    type="button"
                    className="vm-primary"
                    onClick={() => (status === "ready" ? setDismissed(status) : void c.connect())}
                  >
                    {status === "ready" ? "Dismiss" : "Retry now"}
                  </button>
                </>
              )}
            </div>
          </ConnectionNotice>
        )}
      {c.state.active && c.state.grant && c.state.grant.role !== "editor" && (
        <div className="vm-role" role="status">
          ○ {c.state.grant.role === "commenter" ? "Commenter · Comments on" : "Viewer · Read only"}{" "}
          · Design editing is off.{" "}
          <button type="button" onClick={() => c.open()}>
            Open comments
          </button>
        </div>
      )}
      <CommentsPanel controller={c} />
    </section>
  );
}
export function LiveShareSection({ controller: c }: { controller: CollaborationController }) {
  useStore(c.subscribe);
  useEffect(() => {
    void c.loadSharing();
  }, [c]);
  const s = c.state.sharing;
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<Role>("viewer");
  const [confirm, setConfirm] = useState<string | null>(null);
  if (!s)
    return (
      <section className="vm-sharing">
        <h3>Live collaboration</h3>
        <p>{c.state.sharingError ?? "Live collaboration unavailable"}</p>
      </section>
    );
  return (
    <section className="vm-sharing">
      <header>
        <Icon name="cloud" size={14} />
        <h3>Live collaboration</h3>
        <span className="vm-live-badge">✓ {s.enabled ? "On" : "Off"}</span>
        <span className="vm-plan-badge">{s.selfHosted ? "Self-hosted" : "Team"}</span>
      </header>
      <p>
        {s.enabled
          ? "On. Changes appear as people type. Only this document is shared, never your repository."
          : "Off by default. Turning it on shares this document with the team, never your repository."}
      </p>
      <small>Server · {s.server}</small>
      {s.selfHosted && <p>Self-hosted collaboration uses the same presence, comments and roles.</p>}
      {!s.selfHosted && !s.entitled && (
        <p className="vm-warning">
          Team required · Live collaboration requires Team on the hosted service. Snapshot links
          work on every plan.
        </p>
      )}
      <div className="vm-live-toggle">
        <strong>Live collaboration for {c.documentName}</strong>
        <button
          type="button"
          role="switch"
          aria-label="Live collaboration"
          aria-checked={s.enabled}
          disabled={!s.owner || c.state.sharingBusy || (!s.enabled && !s.entitled && !s.selfHosted)}
          onClick={() => (s.enabled ? setConfirm("off") : void c.setLive(true))}
        >
          {s.enabled ? "On" : "Off"}
          <span aria-hidden="true" />
        </button>
      </div>
      {s.enabled && (
        <>
          <div className="vm-member-summary">
            <span>In this document</span>
            <span>
              {s.members.length} people ·{" "}
              {c.participants().filter((p) => p.actor.kind === "agent").length} agents
            </span>
          </div>
          {s.members.map((m) => {
            const participant = c
              .participants()
              .find(
                (p) =>
                  JSON.stringify(p.actor) === m.id ||
                  (m.owner && JSON.stringify(p.actor) === JSON.stringify(c.state.grant?.actor)),
              );
            return (
              <div className="vm-member" key={m.id}>
                {participant ? (
                  <Mark participant={participant} filled={!!m.owner} />
                ) : (
                  <span className="vm-person" data-outline="">
                    {(m.name || m.email || "Person").slice(0, 1)}
                  </span>
                )}
                <span className="vm-member-info">
                  <strong>{m.name || m.email}</strong>
                  <small>
                    {m.owner
                      ? "You · Owner"
                      : `${m.role.charAt(0).toUpperCase() + m.role.slice(1)} · ${m.status === "pending" ? "Invited · Pending" : "Accepted"}`}
                  </small>
                </span>
                {!m.owner && (
                  <select
                    aria-label={`Role for ${m.name || m.email}`}
                    value={m.role}
                    disabled={!s.owner || m.owner || c.state.sharingBusy}
                    onChange={(e) => void c.setRole(m.id, e.currentTarget.value as Role)}
                  >
                    {(["viewer", "commenter", "editor"] as const).map((r) => (
                      <option key={r} value={r}>
                        {r.charAt(0).toUpperCase() + r.slice(1)}
                      </option>
                    ))}
                  </select>
                )}
                {s.owner && !m.owner && (
                  <button
                    type="button"
                    aria-label={`Remove ${m.name || m.email}`}
                    disabled={c.state.sharingBusy}
                    onClick={() => setConfirm(m.id)}
                  >
                    ×
                  </button>
                )}
              </div>
            );
          })}
          {c
            .participants()
            .filter((p) => p.actor.kind === "agent")
            .map((p) => (
              <div className="vm-member" key={p.connection}>
                <Mark participant={p} />
                <span className="vm-member-info">
                  <strong>{p.state?.name ?? "Agent"}</strong>
                  <small>Agent · {p.state?.activity || "Viewing"}</small>
                </span>
                <small className="vm-agent-badge">Agent</small>
              </div>
            ))}
          {s.owner && s.canInvite && (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                void c.invite(email, role);
              }}
            >
              <label>
                Invite by email
                <input
                  type="email"
                  required
                  autoComplete="off"
                  placeholder="name@studio.example"
                  value={email}
                  onChange={(e) => setEmail(e.currentTarget.value)}
                />
              </label>
              <select
                aria-label="Invitation role"
                value={role}
                onChange={(e) => setRole(e.currentTarget.value as Role)}
              >
                <option value="viewer">Viewer</option>
                <option value="commenter">Commenter</option>
                <option value="editor">Editor</option>
              </select>
              <button type="submit" className="vm-primary" disabled={c.state.sharingBusy}>
                Send invite
              </button>
            </form>
          )}
        </>
      )}
      <div className="vm-sharing-notes">
        <p>Delete snapshot link · removes the public copy only.</p>
        <p>Remove collaborator · ends that person’s live access.</p>
        <p>Turn off live collaboration · ends everyone’s live access.</p>
      </div>
      {s.owner && s.enabled && (
        <button
          type="button"
          className="vm-live-off"
          disabled={c.state.sharingBusy}
          onClick={() => setConfirm("off")}
        >
          ⏻ Turn off live collaboration
        </button>
      )}
      {confirm && (
        <div
          className="vm-confirm"
          role="alertdialog"
          aria-label={confirm === "off" ? "Turn off live collaboration" : "Remove collaborator"}
        >
          <p>
            {confirm === "off"
              ? "This ends everyone’s live access. The snapshot link is unchanged."
              : "Their live access ends immediately. The snapshot link is unchanged. Downloaded copies stay with them."}
          </p>
          <button
            type="button"
            disabled={c.state.sharingBusy}
            onClick={() => {
              void (confirm === "off" ? c.setLive(false) : c.remove(confirm));
              setConfirm(null);
            }}
          >
            Remove access
          </button>
          <button type="button" onClick={() => setConfirm(null)}>
            Cancel
          </button>
        </div>
      )}
      {c.state.sharingError && <p role="alert">△ {c.state.sharingError}</p>}
    </section>
  );
}
