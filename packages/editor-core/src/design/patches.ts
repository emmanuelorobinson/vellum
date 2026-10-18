import type { Effect, NodePatch, Paint, Stroke, Value } from "@vellum/types-ts";
import { isVarRef } from "./bindings";
import { addEffect, removeEffect, replaceEffect } from "./effects";
import type { DesignTarget } from "./model";
import { swapPaintOrder } from "./paint";
import { newStroke } from "./stroke";

/** What one field change does to one layer; null when it does not apply or changes nothing. */
export type Edit = (target: DesignTarget) => NodePatch | null;

type Loose = Record<string, unknown>;
const props = (t: DesignTarget) => t.item.node as unknown as Loose;
const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
const kindOf = (t: DesignTarget) => t.item.node.type;
/** Stored properties leave out their default, so a value equal to it becomes `null` (the core's "back to default"). */
const orDefault = <T>(value: T, fallback: T): T | null => (same(value, fallback) ? null : value);
const isFrame = (t: DesignTarget) => kindOf(t) === "frame";

/** Several edits as one: a layer gets one patch, so one click is one transaction. Later edits win where they overlap. */
export const combine =
  (...edits: Edit[]): Edit =>
  (t) => {
    const patches = edits.map((edit) => edit(t)).filter((p): p is NodePatch => p !== null);
    return patches.length ? Object.assign({}, ...patches) : null;
  };

/* ------------------------------------------------------------------------------------------ position */

export const setPosition =
  (axis: "x" | "y", value: number): Edit =>
  (t) => {
    const current = props(t).position as { type?: string; x?: number; y?: number } | undefined;
    const absolute = current?.type === "absolute";
    if (t.inLayout && !absolute) return null;
    const other = axis === "x" ? "y" : "x";
    return {
      position: {
        type: "absolute",
        [axis]: value,
        [other]: absolute ? (current?.[other] ?? 0) : 0,
      },
    } as NodePatch;
  };
export const setRotation =
  (value: Value<number>): Edit =>
  () => ({ rotation: orDefault(value, 0) });

/* ---------------------------------------------------------------------------------------------- size */

export const setSizeMode =
  (axis: "width" | "height", mode: "hug" | "fill" | "fixed"): Edit =>
  (t) => ({
    [axis]: mode === "fixed" ? Math.round(t.item.bounds[axis] * 100) / 100 : mode,
  });
export const setSize =
  (axis: "width" | "height", value: Value<number>): Edit =>
  () => ({ [axis]: isVarRef(value) ? value : Math.max(0, value) });

/* -------------------------------------------------------------------------------------------- layout */

export const setLayout =
  (layout: "none" | "row" | "column" | "grid"): Edit =>
  (t) =>
    isFrame(t) ? { layout: orDefault(layout, "none") } : null;
export const setGap =
  (value: Value<number>): Edit =>
  (t) =>
    isFrame(t) ? { gap: orDefault(value, 0) } : null;

type Sides = [Value<number>, Value<number>, Value<number>, Value<number>];
const sidesOf = (t: DesignTarget): Sides => {
  const raw = props(t).padding as Value<number> | Value<number>[] | undefined;
  if (raw === undefined) return [0, 0, 0, 0];
  if (!Array.isArray(raw)) return [raw, raw, raw, raw];
  return (raw.length === 2 ? [raw[0], raw[1], raw[0], raw[1]] : raw) as Sides;
};
const paddingOf = (sides: Sides): Value<number> | Sides | null =>
  sides.every((s) => same(s, sides[0])) ? orDefault(sides[0], 0) : sides;
export const setPaddingAll =
  (value: Value<number>): Edit =>
  (t) =>
    isFrame(t) ? { padding: orDefault(value, 0) } : null;
export const setPaddingSide =
  (side: 0 | 1 | 2 | 3, value: Value<number>): Edit =>
  (t) => {
    if (!isFrame(t)) return null;
    const sides = [...sidesOf(t)] as Sides;
    sides[side] = value;
    return { padding: paddingOf(sides) } as NodePatch;
  };
export const setPaddingLinked =
  (linked: boolean): Edit =>
  (t) => {
    if (!isFrame(t)) return null;
    const raw = props(t).padding;
    const sides = sidesOf(t);
    if (linked) return Array.isArray(raw) ? { padding: orDefault(sides[0], 0) } : null;
    return Array.isArray(raw) ? null : { padding: sides };
  };
export const setAlign =
  (value: "start" | "center" | "end"): Edit =>
  (t) =>
    isFrame(t) ? { align: orDefault(value, "start") } : null;
export const setJustify =
  (value: "start" | "center" | "end" | "space-between" | "space-around"): Edit =>
  (t) =>
    isFrame(t) ? { justify: orDefault(value, "start") } : null;
export const setGridCount =
  (key: "columns" | "rows", count: number | null): Edit =>
  (t) => {
    if (!isFrame(t) || props(t).layout !== "grid") return null;
    return { [key]: count === null ? null : Math.max(1, Math.round(count)) };
  };

/* ---------------------------------------------------------------------------------------- appearance */

export const setOpacity =
  (value: Value<number>): Edit =>
  () => ({ opacity: orDefault(value, 1) });
export const setRadius =
  (value: Value<number>): Edit =>
  (t) =>
    ["frame", "rect", "polygon"].includes(kindOf(t)) ? { cornerRadius: orDefault(value, 0) } : null;
export const setClip =
  (value: boolean): Edit =>
  (t) =>
    isFrame(t) ? { clip: value ? true : null } : null;
export const setBlend =
  (mode: string): Edit =>
  () => ({ blendMode: orDefault(mode, "normal") });
export const setNotes =
  (text: string): Edit =>
  () => ({ agentNotes: text.trim() ? text : null });

/* ------------------------------------------------------------------------------------------- paints */

const paintsOf = (t: DesignTarget): Paint[] => (props(t).fills as Paint[] | undefined) ?? [];
const setFills = (list: Paint[]): NodePatch => ({ fills: list.length ? list : null });
const unchanged = (t: DesignTarget, key: string, next: unknown) => same(props(t)[key], next);

export const addFill =
  (paint: Paint): Edit =>
  (t) =>
    setFills([...paintsOf(t), paint]);
export const removeFill =
  (index: number): Edit =>
  (t) =>
    paintsOf(t)[index] ? setFills(paintsOf(t).filter((_, i) => i !== index)) : null;
export const updateFill =
  (index: number, update: (paint: Paint) => Paint): Edit =>
  (t) => {
    const list = paintsOf(t);
    const old = list[index];
    if (!old) return null;
    const next = list.map((p, i) => (i === index ? update(p) : p));
    return unchanged(t, "fills", next) ? null : setFills(next);
  };
export const moveFill =
  (from: number, to: number): Edit =>
  (t) => {
    const list = paintsOf(t);
    const next = swapPaintOrder(list, from, to);
    return same(list, next) ? null : setFills(next);
  };
export const setFillColour = (index: number, colour: Value<string>): Edit =>
  updateFill(index, (p) =>
    (p as unknown as Loose).type === "solid" ? ({ ...p, color: colour } as unknown as Paint) : p,
  );

/* ------------------------------------------------------------------------------------------- stroke */

const strokeOf = (t: DesignTarget) => (props(t).stroke as Stroke | null | undefined) ?? null;
export const addStroke =
  (colour: string): Edit =>
  (t) =>
    strokeOf(t) ? null : { stroke: newStroke(colour) as unknown as NodePatch["stroke"] };
export const removeStroke: Edit = (t) => (strokeOf(t) ? { stroke: null } : null);
export const updateStroke =
  (update: (stroke: Stroke) => Stroke): Edit =>
  (t) => {
    const s = strokeOf(t);
    if (!s) return null;
    const next = update(s);
    return same(s, next) ? null : { stroke: next };
  };
export const updateStrokePaint = (index: number, update: (paint: Paint) => Paint): Edit =>
  updateStroke((s) => ({
    ...s,
    paints: s.paints.map((p, i) => (i === index ? update(p) : p)),
  }));
export const addStrokePaint = (paint: Paint): Edit =>
  updateStroke((s) => ({ ...s, paints: [...s.paints, paint] }));
export const removeStrokePaint = (index: number): Edit =>
  updateStroke((s) =>
    s.paints.length > 1 ? { ...s, paints: s.paints.filter((_, i) => i !== index) } : s,
  );

/* ------------------------------------------------------------------------------------------ effects */

const effectsOf = (t: DesignTarget): Effect[] => (props(t).effects as Effect[] | undefined) ?? [];
const setEffects = (list: Effect[]): NodePatch => ({ effects: list.length ? list : null });
export const addEffectTo =
  (effect: Effect): Edit =>
  (t) =>
    setEffects(addEffect(effectsOf(t), effect));
export const removeEffectAt =
  (index: number): Edit =>
  (t) =>
    effectsOf(t)[index] ? setEffects(removeEffect(effectsOf(t), index)) : null;
export const updateEffectAt =
  (index: number, update: (effect: Effect) => Effect): Edit =>
  (t) => {
    const old = effectsOf(t)[index];
    if (!old) return null;
    const next = update(old);
    return same(old, next) ? null : setEffects(replaceEffect(effectsOf(t), index, next));
  };
