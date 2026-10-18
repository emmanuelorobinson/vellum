import {
  EXPORT_FORMATS,
  EXPORT_SCALES,
  type ExportController,
  type ExportFormat,
  type InspectController,
  TOKEN_FORMATS,
  type TokenFormat,
  type TokenSyncController,
} from "@vellum/editor-core";
import { useEffect } from "react";
import { useStore } from "../hooks";
import { ResourceDialog } from "../resources/Dialog";
import { Toast } from "./LedgerLayer";

const SUFFIXES = ["", "@1x", "@2x", "@3x"];
const formatName = (f: TokenFormat) =>
  f === "css" ? "CSS" : f === "tailwind" ? "Tailwind" : "Design Tokens JSON";

function TokenDialog({ sync, readOnly }: { sync: TokenSyncController; readOnly: boolean }) {
  useStore(sync.subscribe);
  const s = sync.state;
  const dialog = s.dialog;
  // Read the file (or render the variables) whenever the dialog opens, so the diff is always on screen.
  useEffect(() => {
    if (dialog) void sync.preview();
  }, [dialog, s.target.format]);
  if (!dialog) return null;
  const importing = dialog === "import";
  const field = (key: "path" | "light" | "dark", label: string, mono = true) => (
    <label className="vl-field">
      <span>{label}</span>
      <input
        className={mono ? "vc-mono" : undefined}
        aria-label={label}
        value={s.target[key]}
        onChange={(e) => sync.set({ [key]: e.target.value })}
        onBlur={() => void sync.preview()}
      />
    </label>
  );
  return (
    <ResourceDialog
      title={
        importing ? `Import tokens from ${formatName(s.target.format)}` : "Export design tokens"
      }
      description={
        importing
          ? "Review the changes before applying them to your file."
          : "Write variables to a file in your workspace."
      }
      width={importing ? 800 : 760}
      className={importing ? "vl-dialog vl-dialog--import" : "vl-dialog"}
      onClose={() => sync.close()}
    >
      {field("path", importing ? "Source" : "Target")}
      {importing ? (
        <label className="vl-field">
          <span>Format</span>
          <select
            aria-label="Format"
            value={s.target.format}
            onChange={(e) => sync.set({ format: e.target.value as TokenFormat })}
          >
            {TOKEN_FORMATS.map((f) => (
              <option key={f.id} value={f.id}>
                {f.label}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <fieldset className="vl-segments">
          <legend className="vl-sr">Format</legend>
          {TOKEN_FORMATS.map((f) => (
            <label key={f.id}>
              <input
                type="radio"
                name="token-format"
                checked={s.target.format === f.id}
                onChange={() => sync.set({ format: f.id })}
              />
              <span>{f.label}</span>
            </label>
          ))}
        </fieldset>
      )}
      {importing ? (
        <div className="vl-pair">
          {field("light", "Light")}
          {field("dark", "Dark")}
        </div>
      ) : (
        <>
          {field("light", "Light")}
          {field("dark", "Dark")}
        </>
      )}
      {importing ? (
        <section className="vl-diff" aria-label="Changes to review">
          <h3>{sync.summary()}</h3>
          {s.plan?.errors.map((e) => (
            <p key={e} className="vl-error" role="alert">
              {e}
            </p>
          ))}
          {s.plan?.changes.map((c) => (
            <div key={`${c.kind}-${c.name}`} className="vl-diff__row" data-kind={c.kind}>
              <span>{c.kind[0]?.toUpperCase() + c.kind.slice(1)}</span>
              <span className="vc-mono">{c.name}</span>
              <span className="vc-mono">
                {c.from ?? "—"} → {c.to ?? "—"}
              </span>
            </div>
          ))}
        </section>
      ) : (
        <pre className="vl-preview vc-mono">{s.exportText}</pre>
      )}
      {s.error ? (
        <p className="vl-error" role="alert">
          {s.error}
        </p>
      ) : null}
      <div className="vl-footer-actions">
        <button type="button" className="vx-button" onClick={() => sync.close()}>
          Cancel
        </button>
        {importing ? (
          <button
            type="button"
            className="vx-button"
            data-primary=""
            disabled={!sync.canApply() || readOnly}
            onClick={() => void sync.apply()}
          >
            Apply
          </button>
        ) : (
          <button
            type="button"
            className="vx-button"
            data-primary=""
            disabled={s.busy || readOnly}
            onClick={() => void sync.write()}
          >
            Write file
          </button>
        )}
      </div>
    </ResourceDialog>
  );
}

function ExportDialog({ exporter }: { exporter: ExportController }) {
  useStore(exporter.subscribe);
  const s = exporter.state;
  if (!s.open) return null;
  const files = exporter.files();
  return (
    <ResourceDialog
      title="Export frames"
      description={exporter.title()}
      width={840}
      className="vl-dialog"
      onClose={() => exporter.close()}
    >
      <label className="vl-field">
        <span>Destination</span>
        <input
          aria-label="Destination"
          className="vc-mono"
          value={s.destination}
          readOnly
          onClick={() => void exporter.choose()}
        />
      </label>
      {s.rows.map((r) => (
        <div key={r.id} className="vl-exportrow">
          <span>{r.name}</span>
          <label>
            <span>Format</span>
            <select
              aria-label={`${r.name} format`}
              value={r.format}
              onChange={(e) => exporter.setRow(r.id, { format: e.target.value as ExportFormat })}
            >
              {EXPORT_FORMATS.map((f) => (
                <option key={f} value={f}>
                  {f.toUpperCase()}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Scale</span>
            <select
              aria-label={`${r.name} scale`}
              value={r.scale}
              disabled={r.format === "svg" || r.format === "pdf"}
              onChange={(e) => exporter.setRow(r.id, { scale: Number(e.target.value) })}
            >
              {[...new Set([...EXPORT_SCALES, r.scale])].map((n) => (
                <option key={n} value={n}>
                  {n}×
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Suffix</span>
            <select
              aria-label={`${r.name} suffix`}
              value={r.suffix}
              onChange={(e) => exporter.setRow(r.id, { suffix: e.target.value })}
            >
              {[...new Set([...SUFFIXES, r.suffix])].map((x) => (
                <option key={x} value={x}>
                  {x || "None"}
                </option>
              ))}
            </select>
          </label>
        </div>
      ))}
      <div className="vl-thumbs">
        {s.rows.map((r, i) => (
          <figure key={r.id}>
            <div
              className="vl-thumb"
              {...(s.thumbnails[r.id]
                ? { style: { backgroundImage: `url(${JSON.stringify(s.thumbnails[r.id])})` } }
                : {})}
            />
            <figcaption className="vc-mono">{files[i]}</figcaption>
          </figure>
        ))}
      </div>
      {s.error ? (
        <p className="vl-error" role="alert">
          {s.error}
        </p>
      ) : null}
      <div className="vl-footer-actions">
        <button type="button" className="vx-button" onClick={() => exporter.close()}>
          Cancel
        </button>
        <button
          type="button"
          className="vx-button"
          data-primary=""
          disabled={!exporter.canWrite()}
          onClick={() => void exporter.write()}
        >
          {exporter.action()}
        </button>
      </div>
    </ResourceDialog>
  );
}

/** The token-sync and Export dialogs, and the toasts they leave behind. */
export function HandoffDialogs({
  tokens,
  exporter,
  inspect,
  readOnly,
  toastLeft,
  onOpenActivity,
}: {
  /** Opens the Activity tab for the batch a token import left behind. */
  onOpenActivity?: () => void;
  inspect?: InspectController | undefined;
  tokens?: TokenSyncController | undefined;
  exporter?: ExportController | undefined;
  readOnly: boolean;
  toastLeft: number;
}) {
  useStore(tokens?.subscribe ?? (() => ({ dispose() {} })));
  useStore(exporter?.subscribe ?? (() => ({ dispose() {} })));
  useStore(inspect?.subscribe ?? (() => ({ dispose() {} })));
  const notice = tokens?.state.notice ?? inspect?.state.notice;
  return (
    <>
      {tokens ? <TokenDialog sync={tokens} readOnly={readOnly} /> : null}
      {exporter ? <ExportDialog exporter={exporter} /> : null}
      {notice ? (
        <Toast
          left={toastLeft}
          title={notice.title}
          body={notice.body}
          {...(tokens?.state.notice && onOpenActivity
            ? {
                action: {
                  label: "Open Activity",
                  run: () => {
                    tokens.dismiss();
                    onOpenActivity();
                  },
                },
              }
            : {})}
          onClose={() => {
            tokens?.dismiss();
            inspect?.dismiss();
          }}
        />
      ) : exporter?.state.notice ? (
        <Toast left={toastLeft} title={exporter.state.notice} onClose={() => exporter.dismiss()} />
      ) : null}
    </>
  );
}
