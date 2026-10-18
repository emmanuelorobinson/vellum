import type { NodeKind, Point } from "@vellum/types-ts";
import { union } from "./geometry";
import type { Box, Overlay, Viewport } from "./tool-types";

/** Handles are screen-space squares: they never scale with the canvas. */
export const HANDLE_SIZE = 8;
/** A mesh point is a coloured circle, 12 px across as in the design. */
export const PAINT_POINT_SIZE = 12;
const FRAME_LABEL_GAP = 6;
const SIZE_LABEL_GAP = 8;

export type OutlineRole = "selection" | "component" | "instance" | "preview";
export type OverlayShape =
  | { kind: "outline"; role: OutlineRole; box: Box }
  | { kind: "marquee"; box: Box }
  | { kind: "handle"; role: "corner" | "point" | "in" | "out"; at: Point; size: number }
  /** A point of a mesh gradient (it carries its colour) or a corner of an image's crop. */
  | {
      kind: "paintPoint";
      role: "mesh" | "crop";
      at: Point;
      size: number;
      colour: string | null;
      selected: boolean;
    }
  | { kind: "line"; role: "guide" | "handle" | "measure"; from: Point; to: Point }
  | {
      kind: "label";
      role: "frame" | "size" | "measure" | "distance";
      text: string;
      at: Point;
      /** Where `at` sits on the text: the frame name hangs from its left edge, the rest are centred. */
      anchor: "start" | "middle";
      selected?: boolean;
    };

export interface SelectedItem {
  box: Box;
  kind: NodeKind;
  /** The root of a main component. */
  component: boolean;
}
export interface OverlayInput {
  /** From the tool controller, in world coordinates. */
  overlay: Overlay;
  viewport: Viewport;
  size: { width: number; height: number };
  selection: readonly SelectedItem[];
  previewBounds: readonly Box[];
  frameLabels: readonly { box: Box; name: string; selected: boolean }[];
  /** The layer under the pointer, for ⌥-hover measurements. */
  hover: Box | null;
  altMeasure: boolean;
  /** Takes path-space points of `overlay.pathAddress` to world points. */
  mapPath?: (point: Point) => Point;
  /** From the paint editor, already in screen pixels. */
  paintHandles?: readonly {
    role: "mesh" | "crop";
    at: Point;
    colour: string | null;
    selected: boolean;
  }[];
}

export const toScreen = (p: Point, v: Viewport): Point => ({
  x: p.x * v.zoom + v.x,
  y: p.y * v.zoom + v.y,
});
const boxToScreen = (b: Box, v: Viewport): Box => ({
  x: b.x * v.zoom + v.x,
  y: b.y * v.zoom + v.y,
  width: b.width * v.zoom,
  height: b.height * v.zoom,
});
const px = (n: number) => `${Math.round(n)} px`;
const overlap = (a0: number, a1: number, b0: number, b1: number) =>
  [Math.max(a0, b0), Math.min(a1, b1)] as const;

export interface DistanceLabel {
  text: string;
  from: Point;
  to: Point;
}
/** Gaps between a selection and the layer under the pointer, in world pixels (⌥-hover). */
export function distanceLabels(a: Box, b: Box): DistanceLabel[] {
  const contains = (o: Box, i: Box) =>
    i.x >= o.x && i.y >= o.y && i.x + i.width <= o.x + o.width && i.y + i.height <= o.y + o.height;
  const edges = (o: Box, i: Box): DistanceLabel[] => {
    const cx = i.x + i.width / 2;
    const cy = i.y + i.height / 2;
    const gaps: [number, Point, Point][] = [
      [i.y - o.y, { x: cx, y: i.y }, { x: cx, y: o.y }],
      [
        o.y + o.height - (i.y + i.height),
        { x: cx, y: i.y + i.height },
        { x: cx, y: o.y + o.height },
      ],
      [i.x - o.x, { x: i.x, y: cy }, { x: o.x, y: cy }],
      [o.x + o.width - (i.x + i.width), { x: i.x + i.width, y: cy }, { x: o.x + o.width, y: cy }],
    ];
    return gaps.map(([gap, from, to]) => ({ text: px(gap), from, to }));
  };
  if (contains(a, b)) return edges(a, b);
  if (contains(b, a)) return edges(b, a);
  const out: DistanceLabel[] = [];
  const [y0, y1] = overlap(a.y, a.y + a.height, b.y, b.y + b.height);
  const [x0, x1] = overlap(a.x, a.x + a.width, b.x, b.x + b.width);
  const midY = y1 >= y0 ? (y0 + y1) / 2 : a.y + a.height / 2;
  const midX = x1 >= x0 ? (x0 + x1) / 2 : a.x + a.width / 2;
  if (a.x + a.width <= b.x)
    out.push({
      text: px(b.x - (a.x + a.width)),
      from: { x: a.x + a.width, y: midY },
      to: { x: b.x, y: midY },
    });
  else if (b.x + b.width <= a.x)
    out.push({
      text: px(a.x - (b.x + b.width)),
      from: { x: b.x + b.width, y: midY },
      to: { x: a.x, y: midY },
    });
  if (a.y + a.height <= b.y)
    out.push({
      text: px(b.y - (a.y + a.height)),
      from: { x: midX, y: a.y + a.height },
      to: { x: midX, y: b.y },
    });
  else if (b.y + b.height <= a.y)
    out.push({
      text: px(a.y - (b.y + b.height)),
      from: { x: midX, y: b.y + b.height },
      to: { x: midX, y: a.y },
    });
  return out;
}

/** Everything the overlay canvas draws, in screen pixels and back to front. Drawing it is the host's job. */
export function overlayShapes(input: OverlayInput): OverlayShape[] {
  const { viewport: v, overlay } = input;
  const shapes: OverlayShape[] = [];
  for (const f of input.frameLabels) {
    const at = toScreen({ x: f.box.x, y: f.box.y }, v);
    shapes.push({
      kind: "label",
      role: "frame",
      text: f.name,
      at: { x: at.x, y: at.y - FRAME_LABEL_GAP },
      anchor: "start",
      selected: f.selected,
    });
  }
  for (const s of input.selection)
    shapes.push({
      kind: "outline",
      role: s.component ? "component" : s.kind === "instance" ? "instance" : "selection",
      box: boxToScreen(s.box, v),
    });
  for (const b of input.previewBounds) {
    const box = boxToScreen(b, v);
    shapes.push({ kind: "outline", role: "preview", box });
    shapes.push({
      kind: "label",
      role: "size",
      text: `${Math.round(b.width)} × ${Math.round(b.height)}`,
      at: { x: box.x + box.width / 2, y: box.y + box.height + SIZE_LABEL_GAP },
      anchor: "middle",
    });
  }
  if (overlay.marquee) shapes.push({ kind: "marquee", box: boxToScreen(overlay.marquee, v) });
  for (const g of overlay.guides) {
    if (g.axis === "x") {
      const x = g.coordinate * v.zoom + v.x;
      shapes.push({
        kind: "line",
        role: "guide",
        from: { x, y: 0 },
        to: { x, y: input.size.height },
      });
    } else {
      const y = g.coordinate * v.zoom + v.y;
      shapes.push({
        kind: "line",
        role: "guide",
        from: { x: 0, y },
        to: { x: input.size.width, y },
      });
    }
    if (g.kind === "equalSpacing" && g.gap !== null) {
      const at =
        g.axis === "x"
          ? { x: g.coordinate * v.zoom + v.x, y: 12 }
          : { x: 24, y: g.coordinate * v.zoom + v.y };
      shapes.push({ kind: "label", role: "distance", text: px(g.gap), at, anchor: "middle" });
    }
  }
  const mapped = (p: Point) =>
    toScreen(overlay.pathAddress && input.mapPath ? input.mapPath(p) : p, v);
  const pointAt = new Map<string, Point>();
  for (const h of overlay.handles)
    if (h.kind === "point" && h.target)
      pointAt.set(`${h.target.contour}:${h.target.index}`, mapped(h.point));
  for (const h of overlay.handles) {
    const at = mapped(h.point);
    if ((h.kind === "in" || h.kind === "out") && h.target) {
      const from = pointAt.get(`${h.target.contour}:${h.target.index}`);
      if (from) shapes.push({ kind: "line", role: "handle", from, to: at });
    }
    shapes.push({ kind: "handle", role: h.kind, at, size: HANDLE_SIZE });
  }
  for (const h of input.paintHandles ?? [])
    shapes.push({
      kind: "paintPoint",
      role: h.role,
      at: h.at,
      size: h.role === "mesh" ? PAINT_POINT_SIZE : HANDLE_SIZE,
      colour: h.colour,
      selected: h.selected,
    });
  if (input.altMeasure && input.hover && input.selection.length) {
    const sel = union(input.selection.map((s) => s.box));
    if (sel)
      for (const d of distanceLabels(sel, input.hover)) {
        const from = toScreen(d.from, v);
        const to = toScreen(d.to, v);
        shapes.push({ kind: "line", role: "measure", from, to });
        shapes.push({
          kind: "label",
          role: "measure",
          text: d.text,
          at: { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 },
          anchor: "middle",
        });
      }
  }
  return shapes;
}
