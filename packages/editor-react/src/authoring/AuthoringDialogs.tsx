import type { AuthoringController, DesignController } from "@vellum/editor-core";
import { setImageFit, setPaintOpacity, updateFill, updateStrokePaint } from "@vellum/editor-core";
import type { Paint } from "@vellum/types-ts";
import { useState } from "react";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
import { ResourceDialog } from "../resources/Dialog";

const safeLink = (url: string) => (/^https?:\/\//i.test(url) ? url : undefined);
export function AuthoringDialogs({
  authoring: a,
  readOnly,
  design,
}: {
  authoring: AuthoringController;
  readOnly: boolean;
  design?: DesignController | undefined;
}) {
  useStore(a.subscribe);
  const s = a.state;
  const [tab, setTab] = useState<"upload" | "stock">("upload");
  const [query, setQuery] = useState("");
  const disabled = readOnly || s.busy;
  if (!s.dialog)
    return s.error ? (
      <div className="vx-action-error" role="alert">
        <Icon name="triangle-alert" />
        {s.error}
        <button
          type="button"
          className="vc-iconbutton"
          aria-label="Dismiss authoring error"
          onClick={() => a.close()}
        >
          ×
        </button>
      </div>
    ) : null;
  if (s.dialog === "guidelines")
    return (
      <ResourceDialog
        title={`${s.guidelines?.name ?? "Design system"} / Guidelines`}
        description="Every agent reads these rules before using this design system."
        width={744}
        onClose={() => a.close()}
        className="vx-guidelines"
      >
        {s.guidelines ? (
          <>
            <textarea
              aria-label="Design-system guidelines"
              className="vx-guidelines-text"
              spellCheck={false}
              readOnly={disabled || !s.guidelines.writable}
              value={s.guidelines.text}
              onChange={(e) => a.editGuidelines(e.target.value)}
            />
            <div className="vx-guidelines-actions">
              <button
                type="button"
                className="vx-button"
                data-primary=""
                disabled={
                  disabled ||
                  !s.guidelines.writable ||
                  !a.host.saveGuidelines ||
                  s.guidelines.text === s.savedText
                }
                onClick={() => void a.saveGuidelines()}
              >
                Save guidelines
              </button>
              <span className="vx-muted" title={s.guidelines.path}>
                {readOnly || !s.guidelines.writable
                  ? "Read-only guidelines"
                  : s.busy
                    ? "Saving…"
                    : s.guidelines.text === s.savedText
                      ? "Stored with the design system"
                      : "Unsaved changes"}
              </span>
            </div>
            <p className="vx-guidelines-location vc-mono">
              {s.guidelines.path}
              {!s.guidelines.writable ? " · Read-only" : ""}
            </p>
          </>
        ) : s.busy ? (
          <p role="status">Loading guidelines…</p>
        ) : null}
        {s.error ? (
          <p role="alert" className="vx-error">
            ⚠ {s.error}
          </p>
        ) : null}
      </ResourceDialog>
    );
  const paint = a.imagePaint();
  const image = paint?.type === "image" ? paint : null;
  const edit = (fn: (p: Paint) => Paint) => a.editImage(fn);
  return (
    <ResourceDialog
      title="Image fill"
      width={344}
      onClose={() => a.close()}
      className={`vx-image-picker vx-image-picker--${tab}`}
    >
      <div className="vx-image-tabs" role="tablist" aria-label="Image source">
        {(["upload", "stock"] as const).map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={tab === t}
            onClick={() => setTab(t)}
          >
            {t === "upload" ? "Upload" : "Stock"}
          </button>
        ))}
        <button
          type="button"
          role="tab"
          aria-selected={false}
          disabled
          title="Image generation · v1.1"
        >
          Generate · v1.1
        </button>
      </div>
      {tab === "upload" ? (
        <>
          <section
            className="vx-upload"
            aria-label="Image upload"
            onDragOver={(e) => {
              if (!disabled && a.host.uploadImage) e.preventDefault();
            }}
            onDrop={(e) => {
              e.preventDefault();
              if (!disabled && e.dataTransfer.files[0])
                void a.importImage("upload", e.dataTransfer.files[0]);
            }}
          >
            <Icon name="image" size={24} />
            <p>Drop an image here, or choose a file.</p>
            <button
              type="button"
              className="vx-button"
              data-primary=""
              disabled={disabled || !a.host.chooseImage}
              onClick={() => void a.importImage("choose")}
            >
              Choose a file
            </button>
          </section>
          <p className="vx-muted">PNG · JPEG · WEBP · SVG</p>
          <label className="vx-image-property">
            Fit
            <select
              aria-label="Image fit"
              disabled={disabled || !image}
              value={image?.fit ?? "fill"}
              onChange={(e) =>
                edit((p) => setImageFit(p, e.target.value as "fill" | "fit" | "stretch"))
              }
            >
              <option value="fill">Fill</option>
              <option value="fit">Fit</option>
              <option value="stretch">Stretch</option>
            </select>
          </label>
          <label className="vx-image-property">
            Opacity
            <input
              key={String(image?.opacity)}
              type="number"
              aria-label="Image opacity"
              min={0}
              max={100}
              defaultValue={typeof image?.opacity === "number" ? image.opacity * 100 : 100}
              disabled={disabled || !image}
              onBlur={(e) => {
                const n = Number(e.target.value);
                if (Number.isFinite(n) && n >= 0 && n <= 100)
                  edit((p) => setPaintOpacity(p, n / 100, (c) => design?.colour(c) ?? String(c)));
              }}
            />
          </label>
        </>
      ) : (
        <>
          <form
            className="vx-stock-search"
            onSubmit={(e) => {
              e.preventDefault();
              void a.search(query);
            }}
          >
            <input
              type="search"
              aria-label="Search stock images"
              placeholder="Search stock images…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <button
              type="submit"
              className="vc-iconbutton"
              aria-label="Search stock"
              disabled={s.searching || !a.host.searchStock || !query.trim()}
            >
              <Icon name="search" />
            </button>
          </form>
          {!a.host.searchStock ? (
            <p role="status" className="vx-muted">
              Stock images are unavailable here.
            </p>
          ) : s.searching ? (
            <p role="status">Searching…</p>
          ) : s.query && !s.photos.length ? (
            <p role="status">No photos found.</p>
          ) : null}
          <div className="vx-stock-grid">
            {s.photos.map((photo) => (
              <figure key={photo.id}>
                <button
                  type="button"
                  aria-label={`Select ${photo.description}`}
                  aria-pressed={s.selected === photo.id}
                  className="vx-stock-photo"
                  onClick={() => a.selectPhoto(photo.id)}
                >
                  <img
                    src={
                      safeLink(photo.previewUrl) ??
                      (photo.previewUrl.startsWith("data:image/") ? photo.previewUrl : undefined)
                    }
                    alt={photo.description}
                    referrerPolicy="no-referrer"
                  />
                </button>
                <figcaption>
                  <a href={safeLink(photo.attribution.sourceUrl)} target="_blank" rel="noreferrer">
                    {photo.attribution.provider}
                  </a>{" "}
                  ·{" "}
                  <a href={safeLink(photo.attribution.authorUrl)} target="_blank" rel="noreferrer">
                    {photo.attribution.author}
                  </a>{" "}
                  ·{" "}
                  <a href={safeLink(photo.attribution.licenceUrl)} target="_blank" rel="noreferrer">
                    {photo.attribution.licence}
                  </a>
                </figcaption>
              </figure>
            ))}
          </div>
          <div className="vx-stock-pages">
            <button
              type="button"
              className="vx-button"
              disabled={s.searching || s.page <= 1}
              onClick={() => void a.search(s.query, s.page - 1)}
            >
              Previous
            </button>
            <span>
              Page {s.page} · {s.total} photos
            </span>
            <button
              type="button"
              className="vx-button"
              disabled={s.searching || !s.photos.length || s.total <= s.page * s.photos.length}
              onClick={() => void a.search(s.query, s.page + 1)}
            >
              Next
            </button>
          </div>
          <button
            type="button"
            className="vx-button"
            data-primary=""
            disabled={disabled || !s.selected || !a.host.importStock}
            onClick={() => void a.importImage("stock")}
          >
            Use selected photo
          </button>
        </>
      )}
      {s.busy ? <p role="status">Importing image…</p> : null}
      {s.error ? (
        <p role="alert" className="vx-error">
          ⚠ {s.error}
        </p>
      ) : null}
    </ResourceDialog>
  );
}
