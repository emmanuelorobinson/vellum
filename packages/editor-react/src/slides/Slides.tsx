import type { SlidesController } from "@vellum/editor-core";
import { useEffect, useRef, useState } from "react";
import { useLocale } from "../accessibility/locale";
import { useStore } from "../hooks";
import "./slides.css";

export function SlideStrip(p: {
  controller: SlidesController;
  selection: readonly string[];
  onSelect(id: string, additive: boolean): void;
  readOnly: boolean;
}) {
  const { t: translateUi } = useLocale();
  useStore(p.controller.subscribe);
  const drag = useRef<string | null>(null);
  const slides = p.controller.frames();
  return (
    <>
      <div className="vc-slides" role="listbox" aria-label="Slides" aria-multiselectable="true">
        {slides.length === 0 ? (
          <p>No slides yet · Draw a frame to add a slide</p>
        ) : (
          slides.map((n, i) => (
            <div
              key={n.id}
              role="option"
              tabIndex={0}
              aria-selected={p.selection.includes(n.id)}
              className="vc-slide"
              draggable={!p.readOnly}
              onDragStart={(e) => {
                drag.current = n.id;
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", n.id);
              }}
              onDragEnd={() => {
                drag.current = null;
              }}
              onDragOver={(e) => {
                if (!p.readOnly && drag.current) {
                  e.preventDefault();
                  e.dataTransfer.dropEffect = "move";
                }
              }}
              onDrop={(e) => {
                e.preventDefault();
                const id = drag.current;
                drag.current = null;
                if (!p.readOnly && id)
                  p.controller.reorder(
                    id,
                    n.id,
                    e.clientY <
                      e.currentTarget.getBoundingClientRect().top + e.currentTarget.clientHeight / 2
                      ? "before"
                      : "after",
                  );
              }}
              onClick={(e) => p.onSelect(n.id, e.metaKey || e.ctrlKey || e.shiftKey)}
              onKeyDown={(e) => {
                if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
                  e.preventDefault();
                  e.stopPropagation();
                  const target = slides[i + (e.key === "ArrowUp" ? -1 : 1)];
                  if (!p.readOnly && target)
                    p.controller.reorder(n.id, target.id, e.key === "ArrowUp" ? "before" : "after");
                } else if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  e.stopPropagation();
                  p.onSelect(n.id, e.metaKey || e.ctrlKey || e.shiftKey);
                }
              }}
              title="Select slide · Drag or Alt+↑ / Alt+↓ to reorder"
            >
              <span className="vc-slide__number">{String(i + 1).padStart(2, "0")}</span>
              <span className="vc-slide__title">{n.name}</span>
            </div>
          ))
        )}
      </div>
      <div className="vc-slide-pages">
        <select
          aria-label={translateUi("Pages")}
          value={p.controller.page()?.id ?? ""}
          onChange={(e) => p.controller.changePage(e.target.value)}
        >
          {p.controller.pages().map((page) => (
            <option key={page.id} value={page.id}>
              {page.name}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="vc-textbutton"
          disabled={p.readOnly}
          onClick={() => p.controller.newPage()}
        >
          {translateUi("New page → Slides")}
        </button>
      </div>
    </>
  );
}
export function SlideActions(p: {
  controller: SlidesController;
  selection: readonly string[];
  shortcut?: string | null;
}) {
  useStore(p.controller.subscribe);
  return (
    <div className="vc-slide-actions" role="toolbar" aria-label="Presentation actions">
      <div className="vc-slide-actions-main">
        <button
          type="button"
          className="vc-slide-present"
          disabled={!p.controller.canPresent()}
          title={
            p.controller.canPresent()
              ? p.shortcut
                ? `Present · ${p.shortcut}`
                : "Present"
              : "Presentation rendering is unavailable"
          }
          onClick={() => p.controller.start(p.selection[0])}
        >
          Present
        </button>
        <button
          type="button"
          disabled={!p.controller.canExport()}
          title={p.controller.canExport() ? undefined : "PDF export is unavailable"}
          onClick={() => void p.controller.exportPdf()}
        >
          Export PDF · slide order
        </button>
      </div>
      {p.selection.some((id) => p.controller.frames().some((n) => n.id === id)) ? (
        <button
          type="button"
          disabled={!p.controller.canExport()}
          onClick={() =>
            void p.controller.exportPdf(
              p.selection.filter((id) => p.controller.frames().some((n) => n.id === id)),
            )
          }
        >
          Export selected slides
        </button>
      ) : null}
      {p.controller.state.busy ? <span role="status">Exporting PDF…</span> : null}
      {p.controller.state.error ? (
        <span role="alert">Error · {p.controller.state.error}</span>
      ) : null}
    </div>
  );
}

/** Only shared-renderer pixels appear here. The editor stays mounted, inert, with its view untouched. */
export function Presentation(p: { controller: SlidesController; revision: number }) {
  useStore(p.controller.subscribe);
  const slide = p.controller.current();
  const root = useRef<HTMLDialogElement>(null);
  const restore = useRef<HTMLElement | null>(null);
  const [image, setImage] = useState<{ id: string; revision: number; url: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const id = slide?.id;
  useEffect(() => {
    restore.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    root.current?.showModal();
    root.current?.focus();
    return () => {
      restore.current?.focus();
    };
  }, []);
  useEffect(() => {
    let ignore = false;
    setImage(null);
    setError(null);
    if (id)
      void p.controller.image(id).then(
        (url) => {
          if (!ignore) {
            if (!/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(url))
              setError("The renderer did not return PNG pixels");
            else setImage({ id, revision: p.revision, url });
          }
        },
        () => {
          if (!ignore) setError("This slide could not be rendered");
        },
      );
    return () => {
      ignore = true;
    };
  }, [p.controller, id, p.revision]);
  if (!slide) return null;
  return (
    <dialog
      ref={root}
      className="vc-presentation"
      aria-modal="true"
      aria-label="Presentation"
      tabIndex={-1}
      onCancel={(e) => {
        e.preventDefault();
        p.controller.stop();
      }}
    >
      {error ? (
        <p role="alert">Error · {error}</p>
      ) : image && image.id === id && image.revision === p.revision ? (
        <img src={image.url} alt={slide.name} />
      ) : (
        <p role="status">Loading slide…</p>
      )}
      <div className="vc-presentation-controls">
        <button
          type="button"
          disabled={p.controller.frames()[0]?.id === id}
          onClick={() => p.controller.previous()}
        >
          Previous slide
        </button>
        <span aria-live="polite">
          {p.controller.frames().findIndex((n) => n.id === id) + 1} / {p.controller.frames().length}{" "}
          · {slide.name}
        </span>
        <button
          type="button"
          disabled={p.controller.frames().at(-1)?.id === id}
          onClick={() => p.controller.next()}
        >
          Next slide
        </button>
        <button type="button" onClick={() => p.controller.stop()}>
          Stop presenting · Esc
        </button>
      </div>
    </dialog>
  );
}
