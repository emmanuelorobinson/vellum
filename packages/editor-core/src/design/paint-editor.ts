import type { Paint, Point } from "@vellum/types-ts";
import { world } from "../geometry";
import { HANDLE_SIZE, toScreen } from "../overlay";
import type { Disposable } from "../registries";
import type { EditorSession } from "../session";
import type { Box, Viewport } from "../tool-types";
import type { DesignController } from "./controller";
import { cropCorners, dragCropCorner, moveMeshPoint, paintKind } from "./paint";
import { updateFill, updateStrokePaint } from "./patches";

export interface PaintTarget {
  scope: "fill" | "stroke";
  index: number;
}
export interface PaintEditorState {
  target: PaintTarget;
  /** The mesh point being edited. */
  point: number | null;
  /** The image's crop handles are on the canvas. */
  cropping: boolean;
}
/** A point to draw and grab on the canvas, in screen pixels. */
export interface PaintHandle {
  role: "mesh" | "crop";
  index: number;
  at: Point;
  /** The colour a mesh point carries; crop corners have none. */
  colour: string | null;
  selected: boolean;
}

type Loose = Record<string, unknown>;
const GRAB = HANDLE_SIZE + 2;
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/**
 * The fill editor's part of the canvas: while a mesh gradient is open its points can be dragged where they are on
 * the shape, and an image's crop corners can be dragged too. A drag is shown at once and written as one
 * transaction on release; Escape (cancel) puts it back.
 */
export function createPaintEditor(deps: { session: EditorSession; design: DesignController }) {
  const { session, design } = deps;
  let state: PaintEditorState | null = null;
  let drag: { role: PaintHandle["role"]; index: number; original: Paint; paint: Paint } | null =
    null;
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const l of [...listeners]) l();
  };

  /** The paint being edited, of the one selected layer, and where that layer is. */
  const current = (): { paint: Paint; bounds: Box } | null => {
    const target = design.single();
    if (!state || !target) return null;
    const node = target.item.node as unknown as Loose;
    const list =
      state.target.scope === "fill"
        ? (node.fills as Paint[] | undefined)
        : (node.stroke as { paints?: Paint[] } | null | undefined)?.paints;
    const paint = list?.[state.target.index];
    return paint ? { paint: drag?.paint ?? paint, bounds: target.item.bounds } : null;
  };
  const toShape = (screen: Point, bounds: Box): Point => {
    const w = world(screen, session.state.viewport);
    return { x: (w.x - bounds.x) / bounds.width, y: (w.y - bounds.y) / bounds.height };
  };
  const fromShape = (p: Point, bounds: Box, view: Viewport): Point =>
    toScreen({ x: bounds.x + p.x * bounds.width, y: bounds.y + p.y * bounds.height }, view);

  const editor = {
    get state(): Readonly<PaintEditorState> | null {
      return state;
    },
    subscribe(listener: () => void): Disposable {
      listeners.add(listener);
      return { dispose: () => void listeners.delete(listener) };
    },
    open(target: PaintTarget) {
      state = { target, point: null, cropping: false };
      drag = null;
      const found = current();
      if (found && paintKind(found.paint) === "mesh") state = { ...state, point: 0 };
      notify();
    },
    close() {
      state = null;
      drag = null;
      notify();
    },
    selectPoint(index: number) {
      const found = current();
      const count =
        ((found?.paint as unknown as Loose | undefined)?.points as unknown[] | undefined)?.length ??
        0;
      if (!state || index < 0 || index >= count) return;
      state = { ...state, point: index };
      notify();
    },
    setCropping(on: boolean) {
      const found = current();
      if (!state) return;
      state = { ...state, cropping: on && !!found && paintKind(found.paint) === "image" };
      notify();
    },
    handles(view: Viewport): PaintHandle[] {
      const found = current();
      if (!state || !found) return [];
      const { paint, bounds } = found;
      const kind = paintKind(paint);
      if (kind === "mesh") {
        const points = ((paint as unknown as Loose).points ?? []) as {
          x: number;
          y: number;
          color: string;
        }[];
        return points.map((p, index) => ({
          role: "mesh",
          index,
          at: fromShape(p, bounds, view),
          colour: design.colour(p.color as never),
          selected: state?.point === index,
        }));
      }
      if (kind === "image" && state.cropping)
        return cropCorners(paint).map((p, index) => ({
          role: "crop",
          index,
          at: fromShape(p, bounds, view),
          colour: null,
          selected: false,
        }));
      return [];
    },
    /** True when the press grabbed a handle, so the canvas leaves it alone. */
    press(screen: Point): boolean {
      const found = current();
      if (!state || !found) return false;
      const view = session.state.viewport;
      let best: PaintHandle | null = null;
      let bestDistance = GRAB;
      for (const h of editor.handles(view)) {
        const d = Math.hypot(h.at.x - screen.x, h.at.y - screen.y);
        if (d <= bestDistance) {
          best = h;
          bestDistance = d;
        }
      }
      if (!best) return false;
      drag = { role: best.role, index: best.index, original: found.paint, paint: found.paint };
      if (best.role === "mesh") state = { ...state, point: best.index };
      notify();
      return true;
    },
    drag(screen: Point): boolean {
      const found = current();
      if (!drag || !found) return false;
      const at = toShape(screen, found.bounds);
      const next =
        drag.role === "mesh"
          ? moveMeshPoint(drag.original, drag.index, at.x, at.y)
          : dragCropCorner(drag.original, drag.index as 0 | 1 | 2 | 3, at);
      drag = { ...drag, paint: next };
      notify();
      return true;
    },
    release(screen: Point): boolean {
      if (!drag) return false;
      editor.drag(screen);
      const done = drag;
      drag = null;
      if (state && !same(done.paint, done.original)) {
        const paint = done.paint;
        design.edit(
          state.target.scope === "fill"
            ? updateFill(state.target.index, () => paint)
            : updateStrokePaint(state.target.index, () => paint),
        );
      }
      notify();
      return true;
    },
    /** True when a drag was put back, so Escape is not also handed to the canvas. */
    cancel(): boolean {
      if (!drag) return false;
      drag = null;
      notify();
      return true;
    },
  };
  return editor;
}
export type PaintEditor = ReturnType<typeof createPaintEditor>;
