import { BROWSER_DEVICES, type BrowserImportController, embedRequest } from "@vellum/editor-core";
import type { BrowserPort } from "@vellum/host-api";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import "./browser.css";
export function BrowserImportPanel({
  controller: c,
  readOnly = false,
  onClose,
}: {
  controller: BrowserImportController;
  readOnly?: boolean;
  onClose?: () => void;
}) {
  const s = useSyncExternalStore(
    c.subscribe,
    () => c.state,
    () => c.state,
  );
  const [picking, setPicking] = useState(false);
  const image = useRef<HTMLImageElement>(null);
  const selection = s.selection;
  return (
    <section className="vb-import" aria-label="Import from browser">
      <header>
        <h2>Import from a local page</h2>
        {onClose ? (
          <button type="button" aria-label="Close import" onClick={onClose}>
            ×
          </button>
        ) : null}
      </header>
      <form
        className="vb-address"
        onSubmit={(e) => {
          e.preventDefault();
          void c.load();
        }}
      >
        <input
          aria-label="Page address"
          spellCheck={false}
          value={s.url}
          onChange={(e) => c.configure({ url: e.target.value })}
        />
        <select
          aria-label="Device size"
          value={s.device}
          onChange={(e) => c.configure({ device: e.target.value as keyof typeof BROWSER_DEVICES })}
        >
          {Object.entries(BROWSER_DEVICES).map(([id, device]) => (
            <option key={id} value={id}>
              {device.label}
            </option>
          ))}
        </select>
        <button type="submit" disabled={s.busy}>
          Open page
        </button>
      </form>
      <div className="vb-preview">
        {s.preview ? (
          <div
            className="vb-preview-content"
            style={{ aspectRatio: `${s.preview.width} / ${s.preview.height}` }}
          >
            <img
              ref={image}
              alt="Browser page preview"
              src={`data:image/png;base64,${s.preview.png}`}
            />
            {picking ? (
              <button
                type="button"
                aria-label="Select an element in the page"
                className="vb-picker"
                onClick={(event) => {
                  const rect = image.current?.getBoundingClientRect();
                  if (!rect || !s.preview) return;
                  c.pick(
                    ((event.clientX - rect.left) * s.preview.width) / rect.width,
                    ((event.clientY - rect.top) * s.preview.height) / rect.height,
                  );
                }}
              />
            ) : null}
            {selection ? (
              <span
                className="vb-selection"
                style={{
                  left: `${(selection.x / s.preview.width) * 100}%`,
                  top: `${(selection.y / s.preview.height) * 100}%`,
                  width: `${(selection.width / s.preview.width) * 100}%`,
                  height: `${(selection.height / s.preview.height) * 100}%`,
                }}
              />
            ) : null}
          </div>
        ) : (
          <p role="status">
            {s.busy ? "◌ Loading page…" : "Open a page to preview and import it."}
          </p>
        )}
      </div>
      <div className="vb-select-row">
        <button
          type="button"
          aria-pressed={picking}
          disabled={!s.preview || s.busy}
          onClick={() => setPicking(!picking)}
        >
          Select an element
        </button>
        {s.preview?.dom ? (
          <select
            aria-label="Page element"
            value={selection?.selector ?? ""}
            onChange={(e) => (e.target.value ? c.select(e.target.value) : c.clearSelection())}
          >
            <option value="">Whole page</option>
            {s.preview.dom.nodes
              .filter((n) => n.selector)
              .map((n, i) => (
                <option key={`${n.selector}-${i}`} value={n.selector}>
                  {n.name}
                </option>
              ))}
          </select>
        ) : null}
        {selection ? <span>{selection.name} selected</span> : null}
      </div>
      <div className="vb-actions">
        <button
          type="button"
          data-primary=""
          disabled={readOnly || s.busy || !s.preview || !c.host.importLayers}
          onClick={() => void c.importLayers()}
        >
          Import as layers
        </button>
        <button
          type="button"
          disabled={readOnly || s.busy || !s.preview || !c.host.importScreenshot}
          onClick={() => void c.screenshot()}
        >
          Screenshot
        </button>
      </div>
      {readOnly || !c.host.importLayers ? (
        <p className="vb-notice">
          This host can preview pages. Import needs an editable document connection.
        </p>
      ) : null}
      {s.error ? (
        <p className="vb-error" role="alert">
          △ {s.error}
        </p>
      ) : null}
      {s.status ? <p role="status">✓ {s.status}</p> : null}
    </section>
  );
}
export interface EmbedView {
  url: string;
  device?: string | null;
  zoom?: number;
  scroll?: { x: number; y: number };
}
export function EmbedProperties({
  node,
  onChange,
  disabled = false,
  onOpen,
}: {
  node: EmbedView;
  onChange(patch: Partial<EmbedView>): void;
  disabled?: boolean;
  onOpen?: () => void;
}) {
  return (
    <section className="vb-properties" aria-label="Embed properties">
      <h3>Embed</h3>
      <label>
        <span>URL</span>
        <input
          aria-label="Embed URL"
          value={node.url}
          disabled={disabled}
          onChange={(e) => onChange({ url: e.target.value })}
        />
      </label>
      <label>
        <span>Device</span>
        <select
          aria-label="Embed device"
          value={node.device ?? "responsive"}
          disabled={disabled}
          onChange={(e) =>
            onChange({ device: e.target.value === "responsive" ? null : e.target.value })
          }
        >
          <option value="responsive">Responsive</option>
          {Object.entries(BROWSER_DEVICES).map(([id, d]) => (
            <option key={id} value={id}>
              {d.label}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span>Zoom</span>
        <input
          aria-label="Embed zoom percent"
          type="number"
          min={10}
          max={400}
          value={Math.round((node.zoom ?? 1) * 100)}
          disabled={disabled}
          onChange={(e) => {
            const value = Number(e.target.value);
            if (value >= 10 && value <= 400) onChange({ zoom: value / 100 });
          }}
        />
      </label>
      <button type="button" disabled={!onOpen} onClick={onOpen}>
        Open page
      </button>
      <p>Live content stays separate from your vector layers.</p>
      {!onOpen ? <p>Page preview is unavailable in this host.</p> : null}
    </section>
  );
}
/** Explicitly opened live session, with a labeled snapshot fallback for hosts without streaming. */
export function EmbedPreview({
  node,
  width,
  height,
  browser,
  openRequest = 0,
}: {
  node: EmbedView;
  width: number;
  height: number;
  browser: BrowserPort;
  openRequest?: number;
}) {
  const [preview, setPreview] = useState<string | null>(null);
  const [live, setLive] = useState(false);
  const watch = useRef<{ dispose(): void } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const revision = useRef(0);
  useEffect(() => {
    revision.current++;
    watch.current?.dispose();
    watch.current = null;
    setLive(false);
    setPreview(null);
    setError(false);
    setBusy(false);
    return () => {
      revision.current++;
      watch.current?.dispose();
      watch.current = null;
    };
  }, [node.url, node.device, node.zoom, node.scroll?.x, node.scroll?.y, width, height]);
  const load = async () => {
    const version = revision.current;
    setBusy(true);
    setError(false);
    try {
      const request = embedRequest(node, { width, height });
      if (browser.watch) {
        const active = await browser.watch(
          request,
          (capture) => {
            if (version === revision.current) {
              setPreview(capture.png);
              setLive(true);
            }
          },
          () => {
            if (version === revision.current) {
              setError(true);
              setLive(false);
            }
          },
        );
        if (version !== revision.current) active.dispose();
        else watch.current = active;
      } else {
        const capture = await browser.capture(request);
        if (version === revision.current) setPreview(capture.png);
      }
    } catch {
      if (version === revision.current) setError(true);
    } finally {
      if (version === revision.current) setBusy(false);
    }
  };
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    if (openRequest) void loadRef.current();
  }, [openRequest]);
  return (
    <section className="vb-embed" style={{ width, height }} aria-label="Website embed">
      {preview ? (
        <img alt="Website preview snapshot" src={`data:image/png;base64,${preview}`} />
      ) : null}
      <button
        type="button"
        disabled={busy}
        onClick={() => {
          if (live) {
            watch.current?.dispose();
            watch.current = null;
            setLive(false);
          } else void load();
        }}
      >
        {busy ? "Loading page…" : live ? "Stop preview" : preview ? "Refresh preview" : "Open page"}
      </button>
      <span role="status">
        {busy
          ? "◌ Loading page…"
          : error
            ? "△ Page unavailable"
            : live
              ? "◉ Live page"
              : preview
                ? "▧ Preview snapshot"
                : "○ Page not loaded"}
      </span>
    </section>
  );
}
