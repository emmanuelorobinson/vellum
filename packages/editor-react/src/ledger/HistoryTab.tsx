import { copy } from "@vellum/config";
import type { HistoryController, LedgerEntry } from "@vellum/editor-core";
import { ActorMarker } from "../chrome/ActorMarker";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";

const changes = (n: number) => `${n} change${n === 1 ? "" : "s"}`;
const clock = (at: number, timeZone: string | undefined) =>
  new Date(at).toLocaleTimeString("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
    ...(timeZone ? { timeZone } : {}),
  });

function Marker({ entry }: { entry: LedgerEntry }) {
  const a = entry.actor;
  if (a.kind === "file")
    return (
      <span className="vl-file" role="img" aria-label="File">
        <Icon name="file-text" size={16} />
      </span>
    );
  return (
    <ActorMarker
      actor={{ id: a.id, kind: a.kind, name: a.name, ...(a.hue ? { hue: a.hue } : {}) }}
      size={16}
    />
  );
}

/** C10: the change ledger. Every accepted batch, newest first, who made it and what it did. */
export function HistoryTab({
  history,
  timeZone,
  readOnly = false,
}: {
  history: HistoryController;
  timeZone?: string;
  readOnly?: boolean;
}) {
  useStore(history.subscribe);
  const s = history.state;
  const entries = history.view();
  const sessions = history.sessions();
  const pages = history.pages();
  return (
    <div className="vl-history">
      <div className="vl-filters">
        <span className="vl-select">
          <select
            aria-label="Session"
            value={s.session}
            onChange={(e) => history.filter({ session: e.target.value })}
          >
            <option value="all">All sessions</option>
            {sessions.map((x) => (
              <option key={x.id} value={x.id}>
                {x.title}
              </option>
            ))}
          </select>
        </span>
        <span className="vl-select">
          <select
            aria-label="Actor"
            value={s.actor}
            onChange={(e) => history.filter({ actor: e.target.value as typeof s.actor })}
          >
            <option value="all">All actors</option>
            <option value="you">You</option>
            <option value="agents">Agents</option>
            <option value="file">File</option>
          </select>
        </span>
        {pages.length > 1 ? (
          <span className="vl-select">
            <select
              aria-label="Page"
              value={s.page}
              onChange={(e) => history.filter({ page: e.target.value })}
            >
              <option value="all">All pages</option>
              {pages.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </span>
        ) : null}
      </div>
      {s.error ? (
        <div className="vl-error" role="alert">
          <Icon name="triangle-alert" size={14} />
          <span>{s.error}</span>
          <button
            type="button"
            className="vc-iconbutton"
            aria-label="Dismiss history error"
            onClick={() => history.dismiss()}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      ) : null}
      {entries.length === 0 ? (
        <p className="vl-empty">{history.empty() ? history.emptyText : "No changes match"}</p>
      ) : (
        <ol className="vl-entries" aria-label="Change ledger">
          {entries.map((e) => {
            const open = s.expanded === e.id;
            return (
              <li
                key={e.id}
                className="vl-entry"
                data-open={open ? "" : undefined}
                data-rejected={e.rejected ? "" : undefined}
                data-entry={e.id}
              >
                <button
                  type="button"
                  className="vl-entry__head"
                  aria-expanded={open}
                  onClick={() => history.toggle(e.id)}
                >
                  <span className="vl-entry__who">
                    <Marker entry={e} />
                    <strong>{e.actor.name}</strong>
                    <span className="vl-entry__meta">
                      {clock(e.at, timeZone)} · {changes(e.count)}
                      {e.actor.kind === "file" ? " applied" : ""}
                    </span>
                  </span>
                  <span className="vl-entry__summary">{e.summary}</span>
                </button>
                {open ? (
                  <div className="vl-entry__body">
                    {e.ops.map((op) => (
                      <div key={op.id} className="vl-op vc-mono">
                        <div>{op.label}</div>
                        {op.detail ? <div>{op.detail}</div> : null}
                      </div>
                    ))}
                    <div className="vl-actions">
                      {e.nodes.length ? (
                        <>
                          <button type="button" onClick={() => history.selectLayers(e.id)}>
                            Select layers
                          </button>
                          <button type="button" onClick={() => history.zoomTo(e.id)}>
                            Zoom to
                          </button>
                        </>
                      ) : null}
                      {history.canUndo(e) ? (
                        <button
                          type="button"
                          disabled={readOnly}
                          onClick={() => void history.undo(e.id)}
                        >
                          Undo this change
                        </button>
                      ) : null}
                      {e.inverse?.length ? (
                        <button
                          type="button"
                          disabled={readOnly}
                          onClick={() => history.askRevert(e.id)}
                        >
                          Revert
                        </button>
                      ) : null}
                      <button type="button" onClick={() => history.copyJson(e.id)}>
                        Copy as JSON
                      </button>
                    </div>
                  </div>
                ) : null}
              </li>
            );
          })}
        </ol>
      )}
      <p className="vl-footer">{copy.historyFooter}</p>
    </div>
  );
}
