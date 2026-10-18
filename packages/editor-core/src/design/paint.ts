import type { GradientKind, Paint, Point, Value } from "@vellum/types-ts";
import { isVarRef } from "./bindings";
import { alphaOf, mixColours, setAlpha } from "./colour";

/** Turns a colour that may be a variable into the colour it is now (the panel supplies the document and mode). */
export type ResolveColour = (colour: Value<string>) => string;
type Loose = Record<string, unknown>;
const loose = (p: Paint) => p as unknown as Loose;
const make = (p: Loose) => p as unknown as Paint;

export type PaintKind = "solid" | GradientKind | "mesh" | "image" | "shader";
export const PAINT_KINDS: readonly PaintKind[] = [
  "solid",
  "linear",
  "radial",
  "angular",
  "mesh",
  "image",
  "shader",
];
const LABELS: Record<PaintKind, string> = {
  solid: "Solid",
  linear: "Linear gradient",
  radial: "Radial gradient",
  angular: "Angular gradient",
  mesh: "Mesh gradient",
  image: "Image",
  shader: "Shader",
};

export function paintKind(p: Paint): PaintKind {
  const l = loose(p);
  if (l.type === "gradient") return l.kind as GradientKind;
  return l.type as PaintKind;
}
export const paintLabel = (p: Paint): string => LABELS[paintKind(p)];
export const kindLabel = (k: PaintKind): string => LABELS[k];
export const isGradient = (p: Paint) => loose(p).type === "gradient";

type Stop = { color: Value<string>; position: number };
type MeshPoint = { x: number; y: number; color: Value<string>; handles?: unknown };
const stopsOf = (p: Paint) => (loose(p).stops as Stop[] | undefined) ?? [];
const pointsOf = (p: Paint) => (loose(p).points as MeshPoint[] | undefined) ?? [];
const clamp01 = (n: number) => Math.min(1, Math.max(0, n));
const round = (n: number, places = 6) => Math.round(n * 10 ** places) / 10 ** places;

/** The colour a paint is best known by, for its swatch and for conversions. */
export function primaryColour(p: Paint): Value<string> | undefined {
  const l = loose(p);
  if (l.type === "solid") return l.color as Value<string>;
  if (l.type === "gradient") return stopsOf(p)[0]?.color;
  if (l.type === "mesh") return pointsOf(p)[0]?.color;
  return undefined;
}
const clear = (c: Value<string>, resolve: ResolveColour): Value<string> =>
  isVarRef(c) ? c : setAlpha(resolve(c), 0);

/** Another kind of paint that keeps what carries over (the colour). Image and shader need a file: null. */
export function convertPaint(
  p: Paint,
  to: PaintKind,
  resolve: ResolveColour = String,
): Paint | null {
  const from = paintKind(p);
  if (from === to) return p;
  if (to === "image" || to === "shader") return null;
  const colour = primaryColour(p);
  const l = loose(p);
  const visible = l.visible === false ? { visible: false } : {};
  if (to === "solid")
    return colour === undefined ? null : make({ type: "solid", color: colour, ...visible });
  if (colour === undefined) return null;
  if (isGradient(p) && to !== "mesh") return make({ ...l, kind: to });
  if (to === "mesh") {
    // A gradient reads left to right: the left corners take its first colour, the right ones its last.
    const first = isGradient(p) ? (stopsOf(p)[0]?.color ?? colour) : colour;
    const last = isGradient(p) ? (stopsOf(p).at(-1)?.color ?? colour) : colour;
    return make({
      type: "mesh",
      columns: 2,
      rows: 2,
      points: [
        { x: 0, y: 0, color: first },
        { x: 1, y: 0, color: last },
        { x: 0, y: 1, color: first },
        { x: 1, y: 1, color: last },
      ],
      ...visible,
    });
  }
  // A solid or a mesh becomes a gradient from its colour to clear.
  return make({
    type: "gradient",
    kind: to,
    stops: [
      { color: colour, position: 0 },
      { color: clear(colour, resolve), position: 1 },
    ],
    ...visible,
  });
}

function colourAt(stops: readonly Stop[], t: number, resolve: ResolveColour): string {
  const first = stops[0];
  const last = stops.at(-1);
  if (!first || !last) return "#000000";
  if (t <= first.position) return resolve(first.color);
  if (t >= last.position) return resolve(last.color);
  for (let i = 1; i < stops.length; i++) {
    const b = stops[i] as Stop;
    const a = stops[i - 1] as Stop;
    if (t <= b.position) {
      const span = b.position - a.position;
      return mixColours(
        resolve(a.color),
        resolve(b.color),
        span === 0 ? 0 : (t - a.position) / span,
      );
    }
  }
  return resolve(last.color);
}

/* ------------------------------------------------------------------------------------------ opacity */

const colours = (p: Paint): Value<string>[] => {
  const l = loose(p);
  if (l.type === "solid") return [l.color as Value<string>];
  if (l.type === "gradient") return stopsOf(p).map((s) => s.color);
  if (l.type === "mesh") return pointsOf(p).map((s) => s.color);
  return [];
};
/** 0 to 1. A solid is its colour's alpha, an image its own opacity, a gradient or mesh its strongest colour. */
export function paintOpacity(p: Paint, resolve: ResolveColour): number {
  const l = loose(p);
  if (l.type === "image") return typeof l.opacity === "number" ? l.opacity : 1;
  const all = colours(p).map((c) => alphaOf(resolve(c)));
  return all.length ? Math.max(...all) : 1;
}
export function setPaintOpacity(p: Paint, opacity: number, resolve: ResolveColour): Paint {
  const o = clamp01(opacity);
  const l = loose(p);
  if (l.type === "image") return make({ ...l, opacity: o });
  if (l.type === "solid") {
    return isVarRef(l.color) ? p : make({ ...l, color: setAlpha(l.color as string, o) });
  }
  const current = paintOpacity(p, resolve);
  const scale = (c: Value<string>): Value<string> => {
    if (isVarRef(c)) return c;
    const a = alphaOf(c);
    return setAlpha(c, current === 0 ? o : (a * o) / current);
  };
  if (l.type === "gradient")
    return make({ ...l, stops: stopsOf(p).map((s) => ({ ...s, color: scale(s.color) })) });
  if (l.type === "mesh")
    return make({ ...l, points: pointsOf(p).map((s) => ({ ...s, color: scale(s.color) })) });
  return p;
}
export function setPaintVisible(p: Paint, visible: boolean): Paint {
  const { visible: _old, ...rest } = loose(p);
  return make(visible ? rest : { ...rest, visible: false });
}
export const paintVisible = (p: Paint) => loose(p).visible !== false;

/* ---------------------------------------------------------------------------------------- gradients */

export function addStop(p: Paint, position: number, resolve: ResolveColour): Paint {
  if (!isGradient(p)) return p;
  const stops = stopsOf(p);
  const at = round(clamp01(position));
  const colour = colourAt(stops, at, resolve);
  const next = [...stops, { color: colour, position: at }].sort((a, b) => a.position - b.position);
  return make({ ...loose(p), stops: next });
}
export function removeStop(p: Paint, index: number): Paint {
  const stops = stopsOf(p);
  if (!isGradient(p) || stops.length <= 2 || !stops[index]) return p;
  return make({ ...loose(p), stops: stops.filter((_, i) => i !== index) });
}
/** Moves one stop; the list stays in order of position, so the stop may end up at another index. */
export function moveStop(
  p: Paint,
  index: number,
  position: number,
): { paint: Paint; index: number } {
  const stops = stopsOf(p);
  const stop = stops[index];
  if (!isGradient(p) || !stop) return { paint: p, index };
  const moved = { ...stop, position: round(clamp01(position)) };
  const rest = stops.filter((_, i) => i !== index);
  let at = rest.findIndex((s) => s.position > moved.position);
  if (at < 0) at = rest.length;
  const next = [...rest.slice(0, at), moved, ...rest.slice(at)];
  return { paint: make({ ...loose(p), stops: next }), index: at };
}
export function setStopColour(p: Paint, index: number, colour: Value<string>): Paint {
  const stops = stopsOf(p);
  if (!isGradient(p) || !stops[index]) return p;
  return make({
    ...loose(p),
    stops: stops.map((s, i) => (i === index ? { ...s, color: colour } : s)),
  });
}
export function setGradientKind(p: Paint, kind: GradientKind): Paint {
  return isGradient(p) ? make({ ...loose(p), kind }) : p;
}
/** Degrees, from the transform's rotation; 0 when there is no transform. */
export function gradientAngle(p: Paint): number {
  const t = loose(p).transform as number[] | undefined;
  if (!t) return 0;
  return round((Math.atan2(t[1] ?? 0, t[0] ?? 1) * 180) / Math.PI, 4);
}
/** A rotation about the centre of the shape, written as the format's 2×3 transform; none for 0. */
export function setGradientAngle(p: Paint, degrees: number): Paint {
  const { transform: _old, ...rest } = loose(p);
  if (!isGradient(p) || degrees === 0) return make(rest);
  const r = (degrees * Math.PI) / 180;
  const cos = Math.cos(r);
  const sin = Math.sin(r);
  return make({
    ...rest,
    transform: [cos, sin, -sin, cos, 0.5 - 0.5 * cos + 0.5 * sin, 0.5 - 0.5 * sin - 0.5 * cos].map(
      (n) => round(n),
    ),
  });
}

/* ------------------------------------------------------------------------------------------- mesh */

export const MESH_MIN = 2;
export const MESH_MAX = 8;
const meshSize = (p: Paint) => ({ columns: Number(loose(p).columns), rows: Number(loose(p).rows) });

export function meshPointLabel(p: Paint, index: number): string {
  const { columns, rows } = meshSize(p);
  const r = Math.floor(index / columns);
  const c = index % columns;
  const v = r === 0 ? "Top" : r === rows - 1 ? "Bottom" : null;
  const h = c === 0 ? "left" : c === columns - 1 ? "right" : null;
  if (v && h) return `${v} ${h}`;
  if (v) return v;
  if (h) return h[0]?.toUpperCase() + h.slice(1);
  return `Row ${r + 1}, column ${c + 1}`;
}
/** A different grid: points evenly spaced, colours sampled from the grid it had. */
export function resizeMesh(
  p: Paint,
  columns: number,
  rows: number,
  resolve: ResolveColour = String,
): Paint {
  if (loose(p).type !== "mesh") return p;
  const cols = Math.min(MESH_MAX, Math.max(MESH_MIN, Math.round(columns)));
  const rws = Math.min(MESH_MAX, Math.max(MESH_MIN, Math.round(rows)));
  const old = meshSize(p);
  if (cols === old.columns && rws === old.rows) return p;
  const pts = pointsOf(p);
  const at = (c: number, r: number) => resolve((pts[r * old.columns + c] as MeshPoint).color);
  const points: MeshPoint[] = [];
  for (let r = 0; r < rws; r++)
    for (let c = 0; c < cols; c++) {
      const u = (c / (cols - 1)) * (old.columns - 1);
      const v = (r / (rws - 1)) * (old.rows - 1);
      const c0 = Math.min(old.columns - 2, Math.floor(u));
      const r0 = Math.min(old.rows - 2, Math.floor(v));
      const top = mixColours(at(c0, r0), at(c0 + 1, r0), u - c0);
      const bottom = mixColours(at(c0, r0 + 1), at(c0 + 1, r0 + 1), u - c0);
      points.push({
        x: round(c / (cols - 1)),
        y: round(r / (rws - 1)),
        color: mixColours(top, bottom, v - r0),
      });
    }
  return make({ ...loose(p), columns: cols, rows: rws, points });
}
export function moveMeshPoint(p: Paint, index: number, x: number, y: number): Paint {
  const pts = pointsOf(p);
  if (!pts[index]) return p;
  return make({
    ...loose(p),
    points: pts.map((pt, i) =>
      i === index ? { ...pt, x: round(clamp01(x)), y: round(clamp01(y)) } : pt,
    ),
  });
}
export function setMeshColour(p: Paint, index: number, colour: Value<string>): Paint {
  const pts = pointsOf(p);
  if (!pts[index]) return p;
  return make({
    ...loose(p),
    points: pts.map((pt, i) => (i === index ? { ...pt, color: colour } : pt)),
  });
}

const byte = (hex: string, at: number) =>
  Number.parseInt(hex.slice(1 + at * 2, 3 + at * 2) || "ff", 16);
/**
 * RGBA pixels of the mesh, for the fill editor's preview. Each cell is the bilinear blend of its four corner
 * colours, laid onto the quad its four points make, so a moved point bends the colours. Bezier handles are not
 * drawn here; the renderer draws the real thing.
 */
export function renderMesh(
  p: Paint,
  width: number,
  height: number,
  resolve: ResolveColour,
): Uint8ClampedArray {
  const out = new Uint8ClampedArray(width * height * 4);
  const best = new Float64Array(width * height).fill(Number.POSITIVE_INFINITY);
  const { columns, rows } = meshSize(p);
  const pts = pointsOf(p);
  if (!pts.length) return out;
  const rgba = pts.map((pt) => {
    const c = resolve(pt.color);
    return [byte(c, 0), byte(c, 1), byte(c, 2), c.length >= 9 ? byte(c, 3) : 255];
  });
  const px = (i: number) => (pts[i] as MeshPoint).x * (width - 1);
  const py = (i: number) => (pts[i] as MeshPoint).y * (height - 1);
  for (let r = 0; r < rows - 1; r++)
    for (let c = 0; c < columns - 1; c++) {
      const i00 = r * columns + c;
      const i10 = i00 + 1;
      const i01 = i00 + columns;
      const i11 = i01 + 1;
      const span = Math.max(
        Math.abs(px(i10) - px(i00)),
        Math.abs(px(i11) - px(i01)),
        Math.abs(py(i01) - py(i00)),
        Math.abs(py(i11) - py(i10)),
        1,
      );
      const steps = Math.ceil(span * 2) + 1;
      for (let a = 0; a <= steps; a++) {
        const s = a / steps;
        for (let b = 0; b <= steps; b++) {
          const t = b / steps;
          const fx =
            px(i00) * (1 - s) * (1 - t) +
            px(i10) * s * (1 - t) +
            px(i01) * (1 - s) * t +
            px(i11) * s * t;
          const fy =
            py(i00) * (1 - s) * (1 - t) +
            py(i10) * s * (1 - t) +
            py(i01) * (1 - s) * t +
            py(i11) * s * t;
          const x = Math.round(fx);
          const y = Math.round(fy);
          if (x < 0 || y < 0 || x >= width || y >= height) continue;
          // Several samples land on one pixel; the one nearest its centre decides, so a point's own pixel
          // carries exactly the point's colour.
          const distance = (fx - x) ** 2 + (fy - y) ** 2;
          if (distance >= (best[y * width + x] as number)) continue;
          best[y * width + x] = distance;
          const at = (y * width + x) * 4;
          for (let k = 0; k < 4; k++) {
            const v =
              (rgba[i00]?.[k] ?? 0) * (1 - s) * (1 - t) +
              (rgba[i10]?.[k] ?? 0) * s * (1 - t) +
              (rgba[i01]?.[k] ?? 0) * (1 - s) * t +
              (rgba[i11]?.[k] ?? 0) * s * t;
            out[at + k] = v;
          }
        }
      }
    }
  return out;
}

/* ------------------------------------------------------------------------------------------ image */

type Crop = [number, number, number, number, number, number];
const IDENTITY: Crop = [1, 0, 0, 1, 0, 0];
const MIN_SCALE = 0.05;
const cropOf = (p: Paint): Crop => (loose(p).crop as Crop | null | undefined) ?? IDENTITY;
export const imageFits = ["fill", "fit", "stretch"] as const;
export function setImageFit(p: Paint, fit: (typeof imageFits)[number]): Paint {
  return make({ ...loose(p), fit });
}
/** The image's four corners in the shape's own 0 to 1 space, clockwise from the top left. */
export function cropCorners(p: Paint): Point[] {
  const [a, b, c, d, e, f] = cropOf(p);
  const at = (x: number, y: number) => ({
    x: round(a * x + c * y + e),
    y: round(b * x + d * y + f),
  });
  return [at(0, 0), at(1, 0), at(1, 1), at(0, 1)];
}
/**
 * Drags one corner of the image with the opposite corner held, keeping the image's shape. Positions are in the
 * shape's own 0 to 1 space. The crop is dropped again when it is the whole box.
 */
export function dragCropCorner(p: Paint, corner: 0 | 1 | 2 | 3, to: Point): Paint {
  const [a, , , d] = cropOf(p);
  const corners = cropCorners(p);
  const fixed = corners[(corner + 2) % 4] as Point;
  // Which way from the fixed corner the dragged one lies.
  const sx = corner === 1 || corner === 2 ? 1 : -1;
  const sy = corner === 2 || corner === 3 ? 1 : -1;
  const k = Math.max(
    MIN_SCALE / Math.min(a, d),
    ((to.x - fixed.x) * sx) / a,
    ((to.y - fixed.y) * sy) / d,
  );
  const w = a * k;
  const h = d * k;
  const e = sx === 1 ? fixed.x : fixed.x - w;
  const f = sy === 1 ? fixed.y : fixed.y - h;
  const next: Crop = [round(w), 0, 0, round(h), round(e), round(f)];
  const { crop: _old, ...rest } = loose(p);
  const whole = next.every((n, i) => n === IDENTITY[i]);
  return make(whole ? rest : { ...rest, crop: next });
}
export function clearCrop(p: Paint): Paint {
  const { crop: _old, ...rest } = loose(p);
  return make(rest);
}

/* ---------------------------------------------------------------------------------------- swatches */

const css = (c: string) => c;
const pct = (n: number) => `${round(n * 100, 2)}%`;
/** What to paint in a small swatch: a colour or a CSS gradient. Null when a swatch cannot show it (an image). */
export function paintSwatch(p: Paint, resolve: ResolveColour): string | null {
  const l = loose(p);
  if (l.type === "solid") return css(resolve(l.color as Value<string>));
  if (l.type === "mesh") {
    const first = pointsOf(p)[0];
    return first ? css(resolve(first.color)) : null;
  }
  if (l.type === "gradient") {
    const list = stopsOf(p)
      .map((s) => `${css(resolve(s.color))} ${pct(s.position)}`)
      .join(", ");
    if (l.kind === "radial") return `radial-gradient(circle, ${list})`;
    if (l.kind === "angular") return `conic-gradient(from 0deg, ${list})`;
    return `linear-gradient(${90 + gradientAngle(p)}deg, ${list})`;
  }
  return null;
}

export function swapPaintOrder<T>(list: readonly T[], from: number, to: number): T[] {
  if (from === to || !list[from] || to < 0 || to >= list.length) return [...list];
  const next = [...list];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved as T);
  return next;
}

/* ---------------------------------------------------------------------------------------- blend modes */

export const BLEND_MODES = [
  "normal",
  "darken",
  "multiply",
  "color-burn",
  "linear-burn",
  "lighten",
  "screen",
  "color-dodge",
  "linear-dodge",
  "overlay",
  "soft-light",
  "hard-light",
  "difference",
  "exclusion",
  "hue",
  "saturation",
  "color",
  "luminosity",
] as const;
export type BlendModeId = (typeof BLEND_MODES)[number];
export function blendLabel(mode: string): string {
  const words = mode.replace(/-/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}
