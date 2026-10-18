import type { Stroke, StrokeAlign, StrokeCap, StrokeJoin, Value } from "@vellum/types-ts";
import { formatField } from "./values";

type Width = Value<number>;
type Sides = [Width, Width, Width, Width];
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** A new stroke: one paint of the colour given, 1 px wide, every other setting at its default. */
export function newStroke(colour: string): Stroke {
  return { paints: [{ type: "solid", color: colour }], width: 1 } as unknown as Stroke;
}

export const SIDE_NAMES = ["Top", "Right", "Bottom", "Left"] as const;
export function strokeSides(stroke: Stroke): Sides {
  const w = stroke.width as Width | Sides;
  return Array.isArray(w) ? (w as Sides) : [w, w, w, w];
}
const asWidth = (sides: Sides): Stroke["width"] =>
  sides.every((s) => same(s, sides[0])) ? sides[0] : sides;
const nonNegative = (n: number) => Math.max(0, n);

/** "1 px" for one width, "Top 1 · Right 1 · Bottom 2 · Left 1" for several. */
export function strokeSummary(stroke: Stroke, resolve: (v: Width) => number): string {
  const sides = strokeSides(stroke);
  if (sides.every((s) => same(s, sides[0])))
    return formatField(resolve(sides[0]), { suffix: " px" });
  return sides.map((s, i) => `${SIDE_NAMES[i]} ${formatField(resolve(s))}`).join(" · ");
}
export function setStrokeWidth(stroke: Stroke, width: number): Stroke {
  return { ...stroke, width: nonNegative(width) };
}
export function setStrokeSide(stroke: Stroke, side: 0 | 1 | 2 | 3, width: number): Stroke {
  const sides = [...strokeSides(stroke)] as Sides;
  sides[side] = nonNegative(width);
  return { ...stroke, width: asWidth(sides) };
}

function setting<K extends "align" | "cap" | "join">(
  stroke: Stroke,
  key: K,
  value: string,
  fallback: string,
): Stroke {
  const { [key]: _old, ...rest } = stroke as unknown as Record<string, unknown>;
  return (value === fallback ? rest : { ...rest, [key]: value }) as unknown as Stroke;
}
export const DEFAULT_ALIGN: StrokeAlign = "center";
export const DEFAULT_CAP: StrokeCap = "butt";
export const DEFAULT_JOIN: StrokeJoin = "miter";
export const setStrokeAlign = (s: Stroke, v: StrokeAlign) => setting(s, "align", v, DEFAULT_ALIGN);
export const setStrokeCap = (s: Stroke, v: StrokeCap) => setting(s, "cap", v, DEFAULT_CAP);
export const setStrokeJoin = (s: Stroke, v: StrokeJoin) => setting(s, "join", v, DEFAULT_JOIN);

export type DashResult = { ok: true; dash: number[] } | { ok: false; message: string };
export function parseDashes(text: string): DashResult {
  const bad: DashResult = { ok: false, message: "Enter lengths such as 4, 4" };
  const trimmed = text.trim();
  if (!trimmed) return { ok: true, dash: [] };
  if (!/^[\d.\s,]+$/.test(trimmed)) return bad;
  const dash = trimmed
    .split(/[\s,]+/)
    .filter(Boolean)
    .map(Number);
  if (dash.some((n) => !Number.isFinite(n) || n < 0) || dash.reduce((a, b) => a + b, 0) <= 0)
    return bad;
  return { ok: true, dash };
}
export const formatDashes = (dash: readonly number[] | undefined): string =>
  dash?.length ? dash.map((n) => formatField(n)).join(", ") : "None";
export function setStrokeDash(stroke: Stroke, dash: number[]): Stroke {
  const { dash: _old, ...rest } = stroke as unknown as Record<string, unknown>;
  return (dash.length ? { ...rest, dash } : rest) as unknown as Stroke;
}
