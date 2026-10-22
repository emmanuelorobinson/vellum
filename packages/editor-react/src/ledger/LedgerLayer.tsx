import type { ConflictController, EditorSession, HistoryController } from "@vellum/editor-core";
import type { Node } from "@vellum/types-ts";
import { useLocale } from "../accessibility/locale";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ResourceDialog } from "../resources/Dialog";

export function Toast({
  title,
  body,
  onClose,
  left,
  action,
}: {
  title: string;
  body?: string;
  onClose(): void;
  left: number;
  action?: { label: string; run(): void };
}) {
  return (
    <div className="vl-toast" role="status" style={{ left }}>
      <strong>{title}</strong>
      {body ? <p>{body}</p> : null}
      {action ? (
        <button type="button" className="vx-button vl-toast__action" onClick={action.run}>
          {action.label}
        </button>
      ) : null}
      <button type="button" className="vc-iconbutton" aria-label="Dismiss" onClick={onClose}>
        <Icon name="x" size={12} />
      </button>
    </div>
  );
}

const props = (node: Node | null, names: readonly string[]): [string, string][] => {
  if (!node) return [];
  const record = node as unknown as Record<string, unknown>;
  const text = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v));
  return names.map((n) => [n, text(record[n])]);
};

/** The conflict resolver (C22-conflict): each version's conflicting properties, then Keep ours / theirs / both. */
function Resolver({ conflicts, readOnly }: { conflicts: ConflictController; readOnly: boolean }) {
  const node = conflicts.state.node;
  if (!node) return null;
  const names = node.properties?.length ? node.properties : ["name", "width", "height"];
  return (
    <ResourceDialog
      title={`Resolve ${node.name}`}
      description="Choose a version, or keep both as separate frames."
      width={600}
      className="vl-resolver"
      onClose={() => conflicts.close()}
    >
      <div className="vl-versions">
        {conflicts.versions().map((v) => (
          <section key={v.side} className="vl-version" aria-label={v.label}>
            <h3>{v.label}</h3>
            {v.node ? (
              <dl>
                {props(v.node, names).map(([k, val]) => (
                  <div key={k}>
                    <dt className="vc-mono">{k}</dt>
                    <dd>{val}</dd>
                  </div>
                ))}
              </dl>
            ) : (
              <p>Removed on this side</p>
            )}
          </section>
        ))}
      </div>
      {conflicts.state.error ? (
        <p className="vl-error" role="alert">
          {conflicts.state.error}
        </p>
      ) : null}
      <div className="vl-resolver__actions">
        <button
          type="button"
          className="vx-button"
          data-primary=""
          disabled={readOnly}
          onClick={() => conflicts.pick("ours")}
        >
          Keep ours
        </button>
        <button
          type="button"
          className="vx-button"
          disabled={readOnly}
          onClick={() => conflicts.pick("theirs")}
        >
          Keep theirs
        </button>
        <button
          type="button"
          className="vx-button"
          disabled={readOnly}
          onClick={() => conflicts.pick("both")}
        >
          Keep both
        </button>
        <button
          type="button"
          className="vl-link"
          disabled={readOnly}
          onClick={() => conflicts.askAgent()}
        >
          Ask agent to merge
        </button>
      </div>
    </ResourceDialog>
  );
}

/**
 * Everything the ledger puts over the editor: the revert prompt and result, the file-changed banner, conflict
 * marks on the canvas and the resolver. `insets` is what docked panels cover, so the toast centres on the canvas.
 */
export function LedgerLayer({
  history,
  conflicts,
  session,
  readOnly,
  insets,
  width,
}: {
  history?: HistoryController | undefined;
  conflicts?: ConflictController | undefined;
  session: EditorSession;
  readOnly: boolean;
  insets: { left: number; right: number };
  width: number;
}) {
  const { t: translateUi } = useLocale();
  useStore(history?.subscribe ?? (() => ({ dispose() {} })));
  useStore(conflicts?.subscribe ?? (() => ({ dispose() {} })));
  useStore(session.subscribe);
  const v = session.state.viewport;
  const banner = conflicts?.banner() ?? null;
  const confirm = history?.state.confirm
    ? history.view().find((e) => e.id === history.state.confirm)
    : undefined;
  const centre = insets.left + (width - insets.left - insets.right) / 2 - 178;
  const marks = conflicts?.conflicts() ?? [];
  const find = (id: string) => session.sceneItem(id);
  return (
    <>
      {marks.map((c) => {
        const item = find(c.id);
        if (!item) return null;
        const box = {
          left: item.bounds.x * v.zoom + v.x,
          top: item.bounds.y * v.zoom + v.y,
          width: item.bounds.width * v.zoom,
          height: item.bounds.height * v.zoom,
        };
        return (
          <div key={c.id} className="vl-conflict" style={box} data-conflict={c.id}>
            <button
              type="button"
              className="vl-conflict__tag"
              onClick={() => conflicts?.open(c.id)}
            >
              Conflict · 2 versions
            </button>
          </div>
        );
      })}
      {banner ? (
        <div className="vl-banner" role="status">
          <span style={{ paddingLeft: insets.left }}>
            <Icon name="info" size={16} />
            {banner.text}
          </span>
          <button type="button" className="vl-link" onClick={() => conflicts?.runBanner()}>
            {banner.action}
          </button>
          <button
            type="button"
            className="vc-iconbutton"
            aria-label="Dismiss banner"
            onClick={() => conflicts?.dismiss()}
          >
            <Icon name="x" size={12} />
          </button>
        </div>
      ) : null}
      {confirm ? (
        <div className="vl-revert" role="alertdialog" aria-label="Revert this change?">
          <h2>Revert this change?</h2>
          <p>
            Later changes depend on this batch. Revert applies inverse changes as a new entry,
            attributed to you.
          </p>
          <div className="vl-revert__actions">
            <button type="button" className="vx-button" onClick={() => history?.cancelRevert()}>
              {translateUi("Cancel")}
            </button>
            <button
              type="button"
              className="vx-button"
              data-primary=""
              disabled={readOnly}
              onClick={() => history?.confirmRevert()}
            >
              Revert
            </button>
          </div>
        </div>
      ) : null}
      {history?.state.notice ? (
        <Toast
          left={centre}
          title={history.state.notice.title}
          body={history.state.notice.body}
          onClose={() => history.dismiss()}
        />
      ) : null}
      {conflicts ? <Resolver conflicts={conflicts} readOnly={readOnly} /> : null}
    </>
  );
}
