import type { FigmaImportController } from "@vellum/editor-core";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import "./figma.css";
export function FigmaImportPanel({
  controller: c,
  readOnly = false,
  onReview,
}: {
  controller: FigmaImportController;
  readOnly?: boolean;
  onReview?(): void;
}) {
  useStore(c.subscribe);
  const s = c.state;
  return (
    <section className="vf-import" aria-label="Import from Figma">
      <h2>{s.report ? "Imported from Figma" : "Import from Figma"}</h2>
      {s.report ? (
        <>
          <div className="vf-summary">
            <Icon name="check" size={16} />
            <span>
              Imported {s.report.frames} frames · {s.report.nodes} layers
            </span>
            <Icon name="check" size={16} />
          </div>
          <p className="vf-review">
            <Icon name={s.report.notes.length ? "triangle-alert" : "check"} size={16} />
            {s.report.notes.length
              ? `${s.report.notes.length} items need review`
              : "Import complete"}
          </p>
          {s.report.notes.map((note, i) => (
            <p key={`${i}-${note}`}>{note}</p>
          ))}
          <button
            type="button"
            className="vx-button"
            data-primary=""
            disabled={!onReview}
            onClick={onReview}
          >
            Review imported frames
          </button>
          <details>
            <summary>View report</summary>
            <pre>{`Imported ${s.report.frames} frames · ${s.report.nodes} layers\n\n${s.report.notes.join("\n")}`}</pre>
          </details>
        </>
      ) : (
        <>
          <label>
            Figma file URL or key
            <input
              aria-label="Figma file URL or key"
              value={s.source}
              disabled={s.busy || readOnly}
              onChange={(e) => c.configure(e.target.value)}
            />
          </label>
          <button
            type="button"
            className="vx-button"
            data-primary=""
            disabled={readOnly || s.busy || !c.host.importFile || !s.source.trim()}
            onClick={() => void c.importFile()}
          >
            Import as layers
          </button>
          <p>Uses the Figma REST API. Local .fig files are not supported.</p>
          {!c.host.importFile ? <p>Figma import is unavailable in this host.</p> : null}
        </>
      )}
      {s.busy ? (
        <p role="status">
          <Icon name="loader-circle" size={16} />
          Importing from Figma…
        </p>
      ) : null}
      {s.error ? (
        <p role="alert">
          <Icon name="triangle-alert" size={16} />
          {s.error}
        </p>
      ) : null}
    </section>
  );
}
