import {
  type EditorSession,
  HANDLE_SIZE,
  overlayShapes,
  type PaintEditor,
  type SelectedItem,
  zoomAbout,
} from "@vellum/editor-core";
import type { Frame, Renderer } from "@vellum/renderer-api";
import type { Value } from "@vellum/types-ts";
import { useEffect, useRef } from "react";
import { useStore } from "../hooks";
import { drawOverlay, readPalette } from "./draw-overlay";
import { TextEditor } from "./TextEditor";

const NO_SUBSCRIPTION = () => ({ dispose() {} });

/** Where the content canvas gets its frames: the engine's display list for the current viewport. */
export interface ContentSource {
  frame(view: {
    width: number;
    height: number;
    dpr: number;
    viewport: { x: number; y: number; zoom: number };
  }): Frame | null;
  subscribe(listener: () => void): { dispose(): void };
}
export type RendererFactory = (canvas: HTMLCanvasElement) => Renderer | Promise<Renderer>;

export interface CanvasHostProps {
  session: EditorSession;
  size: { width: number; height: number };
  componentRoots?: ReadonlySet<string>;
  missingLibraries?: ReadonlyMap<string, boolean>;
  content?: ContentSource;
  createRenderer?: RendererFactory;
  /** The fill editor's points on the canvas: a mesh gradient's points and an image's crop corners. */
  paintEditor?: PaintEditor;
  /** Resolves a colour that may be a variable, for the text being typed. */
  colour?: (value: Value<string>) => string;
  /** Label for assistive technology; the layers panel is the accessible tree. */
  label?: string;
  dpr?: number;
}

const cursorFor = (tool: string, mode: string) =>
  mode === "panning"
    ? "grabbing"
    : tool === "hand"
      ? "grab"
      : tool === "zoom"
        ? "zoom-in"
        : ["frame", "rect", "ellipse", "line", "polygon", "star", "pen", "text", "note"].includes(
              tool,
            )
          ? "crosshair"
          : "default";

export function CanvasHost({
  session,
  size,
  componentRoots,
  missingLibraries,
  content,
  createRenderer,
  paintEditor,
  colour,
  label = "Canvas",
  dpr = typeof devicePixelRatio === "number" ? devicePixelRatio : 1,
}: CanvasHostProps) {
  useStore(session.subscribe);
  useStore(paintEditor ? paintEditor.subscribe : NO_SUBSCRIPTION);
  const root = useRef<HTMLDivElement>(null);
  const contentCanvas = useRef<HTMLCanvasElement>(null);
  const overlayCanvas = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<Renderer | null>(null);
  const alt = useRef(false);

  // The content renderer: created once for this canvas, released with it.
  useEffect(() => {
    const canvas = contentCanvas.current;
    if (!canvas || !createRenderer) return;
    let cancelled = false;
    Promise.resolve(createRenderer(canvas)).then(
      (r) => {
        if (cancelled) r.dispose();
        else {
          renderer.current = r;
          root.current?.setAttribute("data-content", "ready");
          paintContent();
        }
      },
      () => root.current?.setAttribute("data-content", "failed"),
    );
    return () => {
      cancelled = true;
      renderer.current?.dispose();
      renderer.current = null;
    };
  }, [createRenderer]);

  const paintContent = () => {
    const r = renderer.current;
    if (!r || !content || r.state !== "ready") return;
    const frame = content.frame({
      width: size.width,
      height: size.height,
      dpr,
      viewport: session.state.viewport,
    });
    if (!frame) return;
    try {
      r.resize({ width: size.width, height: size.height, dpr });
      r.draw(frame);
    } catch {
      root.current?.setAttribute("data-content", "failed");
    }
  };
  useEffect(() => {
    if (!content) return;
    const s = content.subscribe(paintContent);
    return () => s.dispose();
  }, [content, size.width, size.height]);
  useEffect(paintContent, [size.width, size.height, dpr, session.state.viewport]);

  const shapes = (() => {
    const selection: SelectedItem[] = session.state.selection.flatMap((id) => {
      const item = session.sceneItem(id);
      return item
        ? [{ box: item.bounds, kind: item.node.type, component: componentRoots?.has(id) ?? false }]
        : [];
    });
    const frameLabels = session.roots().flatMap((id) => {
      const item = session.sceneItem(id);
      return item && item.node.type === "frame"
        ? [
            {
              box: item.bounds,
              name: (item.node as { name: string }).name,
              selected: session.state.selection.includes(id),
            },
          ]
        : [];
    });
    const hover = session.hovered ? (session.sceneBounds(session.hovered) ?? null) : null;
    const mapPath = session.overlay.pathAddress
      ? session.pathMapping(session.overlay.pathAddress)
      : null;
    return overlayShapes({
      ...(mapPath ? { mapPath } : {}),
      overlay: session.overlay,
      viewport: session.state.viewport,
      size,
      selection,
      previewBounds: session.state.previewBounds,
      frameLabels,
      hover,
      altMeasure: alt.current,
      ...(paintEditor ? { paintHandles: paintEditor.handles(session.state.viewport) } : {}),
    });
  })();

  useEffect(() => {
    const canvas = overlayCanvas.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    drawOverlay(ctx, shapes, readPalette(canvas), dpr);
  });

  /** A press on an overlay handle carries what it grabbed: a path point or handle, or a scale corner. */
  const handleAt = (p: { x: number; y: number }) => {
    const handles = session.overlay.handles;
    const drawn = shapes.filter((s) => s.kind === "handle");
    const i = drawn.findIndex(
      (h) => h.kind === "handle" && Math.hypot(h.at.x - p.x, h.at.y - p.y) <= HANDLE_SIZE,
    );
    const h = i >= 0 ? handles[i] : undefined;
    if (!h) {
      const target = session.pathHit(p);
      return target ? { target } : {};
    }
    if (h.target) return { target: h.target };
    if (h.kind === "corner" && session.state.mode.type === "scaling") {
      const corners = [
        { x: 0, y: 0 },
        { x: 1, y: 0 },
        { x: 1, y: 1 },
        { x: 0, y: 1 },
      ];
      const corner = corners[i];
      return corner ? { scaleHandle: corner } : {};
    }
    return {};
  };
  const point = (e: { clientX: number; clientY: number }) => {
    const b = root.current?.getBoundingClientRect();
    return { x: e.clientX - (b?.left ?? 0), y: e.clientY - (b?.top ?? 0) };
  };
  const mods = (e: { metaKey: boolean; ctrlKey: boolean; shiftKey: boolean; altKey: boolean }) => ({
    command: e.metaKey || e.ctrlKey,
    shift: e.shiftKey,
    alt: e.altKey,
    canvasFocus: true,
  });

  // Wheel and pinch need a non-passive listener to cancel the page's own scrolling.
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const v = session.state.viewport;
      if (e.ctrlKey || e.metaKey) {
        const factor = Math.exp(-e.deltaY * 0.01);
        session.setViewport(zoomAbout(v, point(e), v.zoom * factor));
      } else if (e.shiftKey) session.setViewport({ ...v, x: v.x - (e.deltaX || e.deltaY) });
      else session.setViewport({ ...v, x: v.x - e.deltaX, y: v.y - e.deltaY });
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  }, [session]);

  return (
    <div
      ref={root}
      className="vc-canvas"
      role="application"
      aria-label={label}
      // biome-ignore lint/a11y/noNoninteractiveTabindex: the canvas is the editing surface and takes the keyboard
      tabIndex={0}
      data-tool={session.state.tool}
      data-content={content && createRenderer ? "loading" : "unavailable"}
      style={{
        cursor: cursorFor(session.state.tool, session.state.mode.type),
        width: size.width,
        height: size.height,
      }}
      onPointerDown={(e) => {
        if (e.button !== 0 && e.button !== 1) return;
        e.currentTarget.focus({ preventScroll: true });
        e.currentTarget.setPointerCapture(e.pointerId);
        const p = point(e);
        // A press on the canvas ends the text being typed, wherever focus was.
        if (session.state.mode.type === "editing") session.dispatch({ type: "finish" });
        // A mesh point or crop corner of the open fill editor is grabbed before the tools see the press.
        if (paintEditor?.press(p)) return;
        session.dispatch({ type: "pointerDown", point: p, ...mods(e), ...handleAt(p) });
      }}
      onPointerMove={(e) => {
        alt.current = e.altKey;
        if (paintEditor?.drag(point(e))) return;
        if (e.altKey) session.hover(point(e));
        session.dispatch({ type: "pointerMove", point: point(e), ...mods(e) });
      }}
      onPointerUp={(e) => {
        if (!paintEditor?.release(point(e)))
          session.dispatch({ type: "pointerUp", point: point(e), ...mods(e) });
        if (e.currentTarget.hasPointerCapture(e.pointerId))
          e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onDoubleClick={(e) =>
        session.dispatch({
          type: "doubleClick",
          point: point(e),
          ...mods(e),
          ...handleAt(point(e)),
        })
      }
    >
      <canvas
        ref={contentCanvas}
        className="vc-canvas__layer"
        width={Math.round(size.width * dpr)}
        height={Math.round(size.height * dpr)}
      />
      <canvas
        ref={overlayCanvas}
        className="vc-canvas__layer vc-canvas__overlay"
        width={Math.round(size.width * dpr)}
        height={Math.round(size.height * dpr)}
        aria-hidden
      />
      {session.state.selection.flatMap((id) => {
        const item = session.sceneItem(id);
        if (
          item?.node.type !== "instance" ||
          !item.node.library ||
          !missingLibraries?.has(item.node.library)
        )
          return [];
        const v = session.state.viewport;
        return [
          <span
            key={id}
            className="vc-library-badge"
            style={{ left: item.bounds.x * v.zoom + v.x, top: item.bounds.y * v.zoom + v.y - 34 }}
          >
            ⚠ Missing library · cached copy
          </span>,
        ];
      })}
      <TextEditor session={session} {...(colour ? { colour } : {})} />
    </div>
  );
}
