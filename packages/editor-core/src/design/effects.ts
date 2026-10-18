import type { Effect, Value } from "@vellum/types-ts";
import { formatField } from "./values";

export type EffectKind = Effect["type"];
export const EFFECT_KINDS: readonly EffectKind[] = [
  "drop-shadow",
  "inner-shadow",
  "layer-blur",
  "background-blur",
  "shader",
];
const LABELS: Record<EffectKind, string> = {
  "drop-shadow": "Drop shadow",
  "inner-shadow": "Inner shadow",
  "layer-blur": "Layer blur",
  "background-blur": "Backdrop blur",
  shader: "Shader",
};
type Loose = Record<string, unknown>;
const loose = (e: Effect) => e as unknown as Loose;
const make = (e: Loose) => e as unknown as Effect;

export const effectLabel = (e: Effect): string => LABELS[e.type];
export const effectLabelOf = (kind: EffectKind): string => LABELS[kind];
export const isShadow = (e: Effect) => e.type === "drop-shadow" || e.type === "inner-shadow";
export const isBlur = (e: Effect) => e.type === "layer-blur" || e.type === "background-blur";
export const effectVisible = (e: Effect) => loose(e).visible !== false;

/** The row's value as the design writes it: "0, 4 · blur 16", "8 px · hidden". */
export function effectSummary(e: Effect, resolve: (v: Value<number>) => number): string {
  const l = loose(e);
  const parts: string[] = [];
  if (isShadow(e)) {
    parts.push(
      `${formatField(resolve(l.x as Value<number>))}, ${formatField(resolve(l.y as Value<number>))}`,
    );
    parts.push(`blur ${formatField(resolve(l.blur as Value<number>))}`);
    const spread = l.spread === undefined ? 0 : resolve(l.spread as Value<number>);
    if (spread !== 0) parts.push(`spread ${formatField(spread)}`);
  } else if (isBlur(e)) {
    parts.push(formatField(resolve(l.radius as Value<number>), { suffix: " px" }));
  } else {
    parts.push(`${String(l.src)} · v1.1`);
    return parts.join(" · ");
  }
  if (l.visible === false) parts.push("hidden");
  return parts.join(" · ");
}

/** A new effect with the design's starting numbers. The shader effect needs a file (v1.1): null. */
export function newEffect(kind: EffectKind, defaults: { shadow: string }): Effect | null {
  switch (kind) {
    case "drop-shadow":
      return make({ type: kind, x: 0, y: 4, blur: 16, color: defaults.shadow });
    case "inner-shadow":
      return make({ type: kind, x: 0, y: 1, blur: 4, color: defaults.shadow });
    case "layer-blur":
      return make({ type: kind, radius: 8 });
    case "background-blur":
      return make({ type: kind, radius: 12 });
    default:
      return null;
  }
}

export const addEffect = (list: readonly Effect[], e: Effect): Effect[] => [...list, e];
export const removeEffect = (list: readonly Effect[], index: number): Effect[] =>
  list.filter((_, i) => i !== index);
export const replaceEffect = (list: readonly Effect[], index: number, e: Effect): Effect[] =>
  list.map((x, i) => (i === index ? e : x));

export function setEffectVisible(e: Effect, visible: boolean): Effect {
  const { visible: _old, ...rest } = loose(e);
  return make(visible ? rest : { ...rest, visible: false });
}
export function setShadowField(
  e: Effect,
  field: "x" | "y" | "blur" | "spread",
  value: number,
): Effect {
  if (!isShadow(e)) return e;
  const v = field === "blur" ? Math.max(0, value) : value;
  const { [field]: _old, ...rest } = loose(e);
  return make(field === "spread" && v === 0 ? rest : { ...rest, [field]: v });
}
export function setEffectColour(e: Effect, colour: Value<string>): Effect {
  return isShadow(e) ? make({ ...loose(e), color: colour }) : e;
}
export function setBlurRadius(e: Effect, radius: number): Effect {
  return isBlur(e) ? make({ ...loose(e), radius: Math.max(0, radius) }) : e;
}
