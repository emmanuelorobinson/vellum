import type { Document, Node, NodePatch, TextRun, Value } from "@vellum/types-ts";
import type { SceneItem } from "../tool-types";
import { type FieldValue, isVarRef, type Mode, readValue, resolveVariable } from "./bindings";

type Run = Record<string, unknown> & { text: string; font: string; size: Value<number> };
const REGULAR = 400;
const round = (n: number, places = 4) => Math.round(n * 10 ** places) / 10 ** places;

const isText = (n: Node) => n.type === "text";
export const runsOf = (n: Node): Run[] =>
  isText(n) ? ((n as { content?: Run[] }).content ?? []) : [];
export const plainText = (n: Node): string =>
  runsOf(n)
    .map((r) => r.text)
    .join("");

export interface TextContext {
  /** Whether a font family can be drawn here. Absent when the host cannot say: every font is trusted. */
  available?: (family: string) => boolean;
  /** The family drawn in place of one that is missing. */
  fallback: string;
}
export interface TextModel {
  font: FieldValue<string>;
  weight: FieldValue<number>;
  size: FieldValue<number>;
  /** A percentage of the font size, or "auto" for the font's own. */
  lineHeight: FieldValue<number | "auto">;
  letterSpacing: FieldValue<number>;
  align: FieldValue<string>;
  resize: FieldValue<string>;
  style: FieldValue<string | null>;
  styles: { id: string; name: string }[];
  /** Families used that cannot be drawn. */
  missing: string[];
  /** The characters when the selection is one layer, or several with the same text. */
  content: string | null;
}

export function fontsInUse(items: readonly SceneItem[]): string[] {
  const out: string[] = [];
  for (const item of items)
    for (const r of runsOf(item.node)) if (!out.includes(r.font)) out.push(r.font);
  return out;
}

export function readText(
  items: readonly SceneItem[],
  doc: Document | null,
  mode: Mode,
  ctx: TextContext,
): TextModel {
  const texts = items.filter((i) => isText(i.node));
  const runs = texts.flatMap((i) => runsOf(i.node));
  const sizeNow = (r: Run): number | undefined => {
    const s = r.size;
    return isVarRef(s) ? (resolveVariable(doc, s.$var, mode) as number | undefined) : (s as number);
  };
  const used = [...new Set(runs.map((r) => r.font))];
  const missing = ctx.available
    ? used.filter((f) => !(ctx.available as (f: string) => boolean)(f))
    : [];
  const shownFont = (r: Run) => (missing.includes(r.font) ? ctx.fallback : r.font);
  const lineHeights = runs.map((r): number | "auto" | { $var: string } | undefined => {
    const lh = r.lineHeight as Value<number> | undefined | null;
    if (lh === undefined || lh === null) return undefined;
    if (isVarRef(lh)) return lh;
    const size = sizeNow(r);
    return size ? round((lh / size) * 100, 2) : "auto";
  });
  const raw = <T>(pick: (n: Record<string, unknown>) => T | undefined) =>
    texts.map((i) => pick(i.node as unknown as Record<string, unknown>));
  const styles = Object.entries(
    (doc?.styles ?? {}) as Record<string, { type: string; name: string }>,
  )
    .filter(([, s]) => s.type === "text")
    .map(([id, s]) => ({ id, name: s.name }));
  const contents = texts.map((i) => plainText(i.node));
  return {
    font: readValue(runs.map(shownFont), doc, mode, ""),
    weight: readValue(
      runs.map((r) => r.weight as number | undefined),
      doc,
      mode,
      REGULAR,
    ),
    size: readValue(
      runs.map((r) => r.size as Value<number>),
      doc,
      mode,
      0,
    ),
    lineHeight: readValue(lineHeights, doc, mode, "auto" as const),
    letterSpacing: readValue(
      runs.map((r) => r.letterSpacing as Value<number> | undefined),
      doc,
      mode,
      0,
    ),
    align: readValue(
      raw((n) => n.align as string | undefined),
      doc,
      mode,
      "left",
    ),
    resize: readValue(
      raw((n) => n.resize as string | undefined),
      doc,
      mode,
      "fixed",
    ),
    style: readValue(
      raw((n) => (n.textStyle as string | null | undefined) ?? null),
      doc,
      mode,
      null,
    ),
    styles,
    missing,
    content:
      contents.length && contents.every((c) => c === contents[0]) ? (contents[0] as string) : null,
  };
}

/* ------------------------------------------------------------------------------------------ patches */

type Edit = (run: Run) => Run;
/** Sets (or, for undefined, removes) one key of a run. */
const put = (run: Run, key: string, value: unknown): Run => {
  const { [key]: _old, ...rest } = run;
  return (value === undefined ? rest : { ...rest, [key]: value }) as Run;
};
const clean = (run: Run): Run =>
  Object.fromEntries(Object.entries(run).filter(([, v]) => v !== undefined)) as Run;
const mapRuns = (item: SceneItem, edit: Edit): NodePatch | null =>
  isText(item.node)
    ? { content: runsOf(item.node).map((r) => clean(edit(r))) as unknown as TextRun[] }
    : null;

export const fontPatch = (item: SceneItem, family: string) =>
  mapRuns(item, (r) => put(r, "font", family));
export const weightPatch = (item: SceneItem, weight: number) =>
  mapRuns(item, (r) => put(r, "weight", weight === REGULAR ? undefined : weight));
export function sizePatch(item: SceneItem, size: number): NodePatch | null {
  return mapRuns(item, (r) => {
    const next = put(r, "size", size);
    const lh = r.lineHeight;
    // The line height is stored in pixels but shown as a proportion of the size, so it follows the size.
    if (typeof lh === "number" && typeof r.size === "number" && r.size > 0)
      return put(next, "lineHeight", round((lh * size) / r.size));
    return next;
  });
}
/** `percent` of each run's own size; null puts the font's own line height back. */
export function lineHeightPatch(item: SceneItem, percent: number | null): NodePatch | null {
  return mapRuns(item, (r) => {
    if (percent === null) return put(r, "lineHeight", undefined);
    return typeof r.size === "number" ? put(r, "lineHeight", round((r.size * percent) / 100)) : r;
  });
}
export const letterSpacingPatch = (item: SceneItem, spacing: number) =>
  mapRuns(item, (r) => put(r, "letterSpacing", spacing === 0 ? undefined : spacing));

const layerPatch = (
  item: SceneItem,
  key: string,
  value: string,
  fallback: string,
): NodePatch | null => (isText(item.node) ? { [key]: value === fallback ? null : value } : null);
export const alignPatch = (item: SceneItem, align: string) =>
  layerPatch(item, "align", align, "left");
export const resizePatch = (item: SceneItem, resize: string) =>
  layerPatch(item, "resize", resize, "fixed");

type StyleBody = Record<string, unknown> & { type: string };
export function stylePatch(
  item: SceneItem,
  id: string | null,
  doc: Document | null,
): NodePatch | null {
  if (!isText(item.node)) return null;
  if (id === null) return { textStyle: null };
  const style = (doc?.styles as Record<string, StyleBody> | undefined)?.[id];
  if (style?.type !== "text") return null;
  const apply: Edit = (r) => {
    let next = put(r, "font", style.font);
    for (const key of [
      "weight",
      "style",
      "size",
      "lineHeight",
      "letterSpacing",
      "underline",
      "strikethrough",
    ])
      next = put(next, key, style[key]);
    return next;
  };
  return { textStyle: id, content: runsOf(item.node).map(apply) as unknown as TextRun[] };
}

/** New characters, in the first run's settings. */
export function contentPatch(item: SceneItem, text: string): NodePatch | null {
  if (!isText(item.node) || plainText(item.node) === text) return null;
  const first = runsOf(item.node)[0];
  if (!first) return null;
  return { content: [{ ...first, text }] as unknown as TextRun[] };
}
